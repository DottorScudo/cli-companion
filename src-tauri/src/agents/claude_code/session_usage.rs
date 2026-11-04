//! Consumi di sessione di Claude Code: token letti dal transcript.
//!
//! I transcript `projects/<slug>/<id>.jsonl` (e i sub-agent in
//! `<id>/subagents/*.jsonl`) sono append-only; la cache tiene per ogni file
//! l'offset letto, cosi' ad ogni chiamata si leggono solo i byte nuovi.

use super::ClaudeCode;
use crate::agents::{SessionUsage, SessionUsageReader, TokenTotals};
use crate::platform::{wsl_to_windows_path, Target};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Oltre questo numero di file in cache la si svuota (sessioni vecchie).
const CACHE_MAX_FILES: usize = 512;

#[derive(Debug, Clone, Copy)]
struct MsgUsage {
    totals: TokenTotals,
    sidechain: bool,
}

/// Stato di lettura di un file `.jsonl`.
#[derive(Default)]
struct FileState {
    /// Byte gia' elaborati (fine dell'ultima riga completa).
    offset: u64,
    /// Lunghezza vista all'ultima lettura: se cala, il file e' stato sostituito.
    len: u64,
    /// Ultimo uso visto per ogni id messaggio (un messaggio puo' stare su
    /// piu' righe: vale l'ultima).
    msgs: HashMap<String, MsgUsage>,
    /// Id e modello dell'ultimo messaggio della catena principale.
    last_main: Option<(String, Option<String>)>,
}

/// Cache dei transcript letti. Un lock per file: quello globale serve solo a
/// prendere/inserire/sfrattare le voci, la lettura del disco non serializza
/// le sessioni.
#[derive(Default)]
struct TranscriptCache(Mutex<HashMap<PathBuf, Arc<Mutex<FileState>>>>);

impl TranscriptCache {
    /// Stato del file `path` (creato se manca); sfratta tutto oltre il limite.
    fn entry(&self, path: &Path) -> Arc<Mutex<FileState>> {
        let mut files = self.0.lock().unwrap();
        if files.len() > CACHE_MAX_FILES {
            files.clear();
        }
        files.entry(path.to_path_buf()).or_default().clone()
    }
}

fn token_field(usage: &Value, key: &str) -> u64 {
    usage.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn process_line(state: &mut FileState, line: &[u8], line_offset: u64) {
    let Ok(text) = std::str::from_utf8(line) else {
        return;
    };
    // Scarto veloce: solo le righe assistant con usage interessano.
    if !text.contains("\"usage\"") {
        return;
    }
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return;
    };
    if v.get("type").and_then(Value::as_str) != Some("assistant") {
        return;
    }
    let Some(msg) = v.get("message") else {
        return;
    };
    let Some(usage) = msg.get("usage").filter(|u| u.is_object()) else {
        return;
    };
    let model = msg.get("model").and_then(Value::as_str);
    // Messaggi sintetici (errori locali): niente token, falserebbero il contesto.
    if model == Some("<synthetic>") {
        return;
    }
    let id = msg.get("id").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("anon:{line_offset}"));
    let sidechain = v.get("isSidechain").and_then(Value::as_bool).unwrap_or(false);
    let totals = TokenTotals {
        input_tokens: token_field(usage, "input_tokens"),
        output_tokens: token_field(usage, "output_tokens"),
        cache_read_tokens: token_field(usage, "cache_read_input_tokens"),
        cache_creation_tokens: token_field(usage, "cache_creation_input_tokens"),
    };
    if !sidechain {
        state.last_main = Some((id.clone(), model.map(str::to_string)));
    }
    state.msgs.insert(id, MsgUsage { totals, sidechain });
}

/// Legge i byte nuovi di `path` nello stato. Una riga senza `\n` finale (il
/// file e' in scrittura) non viene consumata: si rilegge alla chiamata dopo.
fn refresh(state: &mut FileState, path: &Path) -> std::io::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    if len < state.len || len < state.offset {
        *state = FileState::default();
    }
    state.len = len;
    if len == state.offset {
        return Ok(());
    }
    file.seek(SeekFrom::Start(state.offset))?;
    let mut reader = BufReader::new(file.take(len - state.offset));
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let n = reader.read_until(b'\n', &mut buf)?;
        if n == 0 || buf.last() != Some(&b'\n') {
            break;
        }
        process_line(state, &buf, state.offset);
        state.offset += n as u64;
    }
    Ok(())
}

fn add(a: &mut TokenTotals, b: &TokenTotals) {
    a.input_tokens += b.input_tokens;
    a.output_tokens += b.output_tokens;
    a.cache_read_tokens += b.cache_read_tokens;
    a.cache_creation_tokens += b.cache_creation_tokens;
}

/// Consumi di `transcript` (e dei sub-agent nella cartella accanto),
/// aggiornando `cache` in modo incrementale.
fn read_usage(cache: &TranscriptCache, transcript: &Path) -> Result<SessionUsage, String> {
    let main = cache.entry(transcript);
    let mut main = main.lock().unwrap();
    refresh(&mut main, transcript).map_err(|e| format!("Transcript non leggibile: {e}"))?;

    let mut usage = SessionUsage::default();
    let mut count = 0u32;
    for m in main.msgs.values() {
        if m.sidechain {
            add(&mut usage.subagents, &m.totals);
        } else {
            add(&mut usage.main, &m.totals);
            count += 1;
        }
    }
    usage.messages = count;
    if let Some((id, model)) = &main.last_main {
        if let Some(m) = main.msgs.get(id) {
            usage.context_tokens = m.totals.input_tokens + m.totals.cache_read_tokens + m.totals.cache_creation_tokens;
        }
        usage.model = model.clone();
    }

    let sub_dir = transcript.with_extension("").join("subagents");
    if let Ok(entries) = std::fs::read_dir(&sub_dir) {
        for path in entries.filter_map(|e| e.ok()).map(|e| e.path()) {
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let state = cache.entry(&path);
            let mut state = state.lock().unwrap();
            if refresh(&mut state, &path).is_err() {
                continue;
            }
            for m in state.msgs.values() {
                add(&mut usage.subagents, &m.totals);
            }
        }
    }
    Ok(usage)
}

/// Controllo sintattico del path del transcript, come lo vede l'agente
/// (Windows o Linux): niente `..`, estensione `.jsonl` e forma
/// `.../projects/<slug>/<id>.jsonl`.
fn validate_transcript_path(p: &str) -> Result<(), String> {
    let comps: Vec<&str> = p.trim().split(['/', '\\']).filter(|c| !c.is_empty()).collect();
    let bad = || Err("Percorso del transcript non valido.".to_string());
    if p.contains('\0') || comps.contains(&"..") {
        return bad();
    }
    let n = comps.len();
    let file_ok = comps.last().is_some_and(|f| f.len() > ".jsonl".len() && f.ends_with(".jsonl"));
    if !file_ok || n < 3 || comps[n - 3] != "projects" {
        return bad();
    }
    Ok(())
}

/// Path del transcript leggibile dall'host: invariato su Windows, UNC (o
/// `C:\` per `/mnt/c`) per una distro WSL.
fn resolve_transcript_path(env: &Target, p: &str) -> Result<PathBuf, String> {
    validate_transcript_path(p)?;
    let p = p.trim();
    match env {
        Target::Windows if p.starts_with('/') => Err("Percorso del transcript non valido.".to_string()),
        Target::Windows => Ok(PathBuf::from(p)),
        Target::Wsl { distro: d } => wsl_to_windows_path(d, p)
            .map(PathBuf::from)
            .ok_or_else(|| "Percorso del transcript non valido.".to_string()),
    }
}

/// Consumi di token della sessione con transcript `transcript_path`.
fn session_usage(env: &Target, transcript_path: &str, cache: &TranscriptCache) -> Result<SessionUsage, String> {
    let path = resolve_transcript_path(env, transcript_path)?;
    read_usage(cache, &path)
}

/// Cache condivisa da tutte le chiamate dell'app: dettaglio interno dell'integrazione.
fn shared_cache() -> &'static TranscriptCache {
    static CACHE: OnceLock<TranscriptCache> = OnceLock::new();
    CACHE.get_or_init(TranscriptCache::default)
}

impl SessionUsageReader for ClaudeCode {
    fn session_usage(&self, target: &Target, transcript_path: &str) -> Result<SessionUsage, String> {
        session_usage(target, transcript_path, shared_cache())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mc-usage-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn line(id: &str, side: bool, usage: (u64, u64, u64, u64), model: &str) -> String {
        format!(
            "{}\n",
            serde_json::json!({
                "type": "assistant",
                "isSidechain": side,
                "message": {"id": id, "model": model, "usage": {
                    "input_tokens": usage.0, "output_tokens": usage.1,
                    "cache_read_input_tokens": usage.2, "cache_creation_input_tokens": usage.3}}
            })
        )
    }

    fn append(path: &Path, text: &str) {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        f.write_all(text.as_bytes()).unwrap();
    }

    fn session_file(dir: &Path) -> PathBuf {
        let p = dir.join("projects").join("slug").join("sess.jsonl");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        p
    }

    #[test]
    fn duplicate_ids_keep_last_usage_and_context_from_last_main() {
        let dir = tmp("dup");
        let p = session_file(&dir);
        append(&p, &line("a", false, (2, 10, 100, 5), "claude-opus-5-5"));
        append(&p, &line("a", false, (2, 182, 100, 5), "claude-opus-5-5"));
        append(&p, &line("b", false, (3, 20, 300, 50), "claude-sonnet-5-5"));
        let cache = TranscriptCache::default();
        let u = read_usage(&cache, &p).unwrap();
        assert_eq!(u.messages, 2);
        assert_eq!(
            u.main,
            TokenTotals { input_tokens: 5, output_tokens: 202, cache_read_tokens: 400, cache_creation_tokens: 55 }
        );
        assert_eq!(u.context_tokens, 3 + 300 + 50);
        assert_eq!(u.model.as_deref(), Some("claude-sonnet-5-5"));
        assert_eq!(u.subagents, TokenTotals::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_id_updated_in_a_later_read() {
        let dir = tmp("dup-later");
        let p = session_file(&dir);
        let cache = TranscriptCache::default();
        append(&p, &line("a", false, (1, 5, 0, 0), "m"));
        assert_eq!(read_usage(&cache, &p).unwrap().main.output_tokens, 5);
        append(&p, &line("a", false, (1, 90, 0, 0), "m"));
        let u = read_usage(&cache, &p).unwrap();
        assert_eq!((u.main.output_tokens, u.messages), (90, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn partial_trailing_line_is_reread_after_append() {
        let dir = tmp("partial");
        let p = session_file(&dir);
        let cache = TranscriptCache::default();
        let second = line("b", false, (1, 7, 0, 0), "m");
        let (head, tail) = second.split_at(second.len() / 2);
        append(&p, &line("a", false, (1, 3, 0, 0), "m"));
        append(&p, head);
        let u = read_usage(&cache, &p).unwrap();
        assert_eq!((u.messages, u.main.output_tokens), (1, 3));
        // Ancora incompleta: invariato.
        assert_eq!(read_usage(&cache, &p).unwrap().messages, 1);
        append(&p, tail);
        let u = read_usage(&cache, &p).unwrap();
        assert_eq!((u.messages, u.main.output_tokens), (2, 10));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_and_irrelevant_lines_are_skipped() {
        let dir = tmp("bad");
        let p = session_file(&dir);
        append(&p, "non json {\"usage\"\n");
        append(&p, "\n");
        append(&p, "{\"type\":\"user\",\"message\":{\"usage\":{\"input_tokens\":999}}}\n");
        append(&p, "{\"type\":\"assistant\",\"message\":{\"id\":\"x\",\"usage\":5}}\n");
        append(&p, "{\"type\":\"assistant\",\"message\":{\"id\":\"y\",\"usage\":{\"input_tokens\":\"7\"}}}\n");
        append(&p, &line("<synth>", false, (0, 0, 0, 0), "<synthetic>"));
        append(&p, &line("ok", false, (4, 6, 0, 0), "m"));
        let u = read_usage(&TranscriptCache::default(), &p).unwrap();
        // "y" ha un usage oggetto con campo di tipo sbagliato: conta 0 token ma e' un messaggio.
        assert_eq!(u.main.input_tokens, 4);
        assert_eq!(u.main.output_tokens, 6);
        assert_eq!(u.model.as_deref(), Some("m"));
        assert_eq!(u.context_tokens, 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sidechain_and_subagent_files_go_to_subagents() {
        let dir = tmp("sub");
        let p = session_file(&dir);
        append(&p, &line("m1", false, (1, 10, 0, 0), "m"));
        append(&p, &line("s1", true, (2, 20, 0, 0), "m"));
        let subs = p.with_extension("").join("subagents");
        std::fs::create_dir_all(&subs).unwrap();
        append(&subs.join("agent-1.jsonl"), &line("a1", true, (3, 30, 0, 0), "m"));
        append(&subs.join("agent-1.jsonl"), &line("a1", true, (3, 33, 0, 0), "m"));
        append(&subs.join("agent-2.jsonl"), &line("a2", true, (4, 40, 0, 0), "m"));
        append(&subs.join("note.txt"), &line("zz", true, (100, 100, 0, 0), "m"));
        let cache = TranscriptCache::default();
        let u = read_usage(&cache, &p).unwrap();
        assert_eq!((u.main.input_tokens, u.main.output_tokens, u.messages), (1, 10, 1));
        assert_eq!((u.subagents.input_tokens, u.subagents.output_tokens), (2 + 3 + 4, 20 + 33 + 40));
        // Un sub-agent cresce: letto in modo incrementale.
        append(&subs.join("agent-2.jsonl"), &line("a3", true, (1, 1, 0, 0), "m"));
        assert_eq!(read_usage(&cache, &p).unwrap().subagents.output_tokens, 20 + 33 + 40 + 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shrunk_or_replaced_file_resets_state() {
        let dir = tmp("shrink");
        let p = session_file(&dir);
        let cache = TranscriptCache::default();
        append(&p, &line("a", false, (1, 100, 0, 0), "m"));
        append(&p, &line("b", false, (1, 100, 0, 0), "m"));
        assert_eq!(read_usage(&cache, &p).unwrap().messages, 2);
        std::fs::write(&p, line("c", false, (9, 1, 0, 0), "m2")).unwrap();
        let u = read_usage(&cache, &p).unwrap();
        assert_eq!((u.messages, u.main.input_tokens, u.main.output_tokens), (1, 9, 1));
        assert_eq!(u.model.as_deref(), Some("m2"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_transcript_is_an_error() {
        let dir = tmp("missing");
        let p = session_file(&dir);
        assert!(read_usage(&TranscriptCache::default(), &p).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- path ---

    #[test]
    fn transcript_path_validation() {
        for ok in [
            r"C:\Users\me\.claude\projects\C--x\abc.jsonl",
            "C:/Users/me/.claude/projects/slug/abc.jsonl",
            "/home/u/.claude/projects/-home-u-p/abc-123.jsonl",
        ] {
            assert!(validate_transcript_path(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "abc.jsonl",
            "/home/u/.claude/projects/abc.jsonl",
            "/home/u/.claude/other/slug/abc.jsonl",
            "/home/u/.claude/projects/slug/abc.json",
            "/home/u/.claude/projects/slug/abc.jsonl.txt",
            "/home/u/.claude/projects/slug/.jsonl",
            "/home/u/.claude/projects/../../etc/x/abc.jsonl",
            r"C:\Users\me\.claude\projects\..\slug\abc.jsonl",
            "/home/u/.claude/projects/slug/\0abc.jsonl",
        ] {
            assert!(validate_transcript_path(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn transcript_path_resolution_per_env() {
        let wsl = Target::Wsl { distro: "Ubuntu".into() };
        assert_eq!(
            resolve_transcript_path(&wsl, "/home/u/.claude/projects/s/a.jsonl").unwrap(),
            PathBuf::from(r"\\wsl.localhost\Ubuntu\home\u\.claude\projects\s\a.jsonl")
        );
        assert_eq!(
            resolve_transcript_path(&wsl, "/mnt/c/Users/me/.claude/projects/s/a.jsonl").unwrap(),
            PathBuf::from(r"C:\Users\me\.claude\projects\s\a.jsonl")
        );
        assert!(resolve_transcript_path(&wsl, r"C:\x\projects\s\a.jsonl").is_err());
        assert_eq!(
            resolve_transcript_path(&Target::Windows, r"C:\u\.claude\projects\s\a.jsonl").unwrap(),
            PathBuf::from(r"C:\u\.claude\projects\s\a.jsonl")
        );
        assert!(resolve_transcript_path(&Target::Windows, "/home/u/.claude/projects/s/a.jsonl").is_err());
        assert!(resolve_transcript_path(&wsl, "/home/u/../x/projects/s/a.jsonl").is_err());
    }
}
