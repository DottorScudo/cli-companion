//! Test architetturale: il core non nomina mai un agente specifico. Tutto cio'
//! che e' proprio di un agente (id, comandi, moduli) sta in
//! `agents/<cartella>/`; fuori da li' si passa dai contratti. Fa eccezione solo
//! la registrazione del modulo (`agents/mod.rs`, `agents/registry.rs`).
//!
//! Si leggono i soli identificatori e stringhe del codice di produzione:
//! restano fuori commenti, test e moduli dichiarati `#[cfg(test)] mod x;`.
//! Come aggiungere un agente: `docs/wiki/integrare-un-coding-agent.md`.

mod scanner;

use super::registry;
use scanner::{scan, Token, TokenKind};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Guida da leggere quando il test fallisce.
const WIKI_PAGE: &str = "docs/wiki/integrare-un-coding-agent.md";

/// Un'integrazione e cio' che la identifica nel codice.
struct Integration {
    /// Cartella (e modulo) sotto `agents/`.
    folder: &'static str,
    /// Id nell'API e nei dati salvati: nessuna stringa fuori dalla cartella lo contiene.
    id: &'static str,
    /// Programmi dell'agente: nessuna stringa fuori dalla cartella e' uguale a uno di questi.
    commands: &'static [&'static str],
}

/// Integrazioni note al test. Aggiungendo un agente: una riga qui.
const INTEGRATIONS: &[Integration] =
    &[Integration { folder: "claude_code", id: "claude-code", commands: &["claude", "claude.exe"] }];

/// Cartelle di `agents/` che non sono integrazioni.
const CORE_FOLDERS: [&str; 3] = ["architecture", "contracts", "presets"];

/// File che dichiarano e registrano i moduli delle integrazioni.
const REGISTRATION_FILES: [&str; 2] = ["agents/mod.rs", "agents/registry.rs"];

/// Un riferimento a un'integrazione in un punto in cui non dovrebbe esserci.
#[derive(Debug, PartialEq, Eq)]
struct Violation {
    file: String,
    line: usize,
    what: String,
}

fn agents_folder_of(path: &str) -> Option<&str> {
    path.strip_prefix("agents/")?.split('/').next().filter(|_| path.matches('/').count() >= 2)
}

/// Riferimenti a integrazioni nei token del file `path` (relativo a `src/`,
/// slash dritti), esclusi quelli della cartella di ciascuna integrazione.
fn violations_in(path: &str, tokens: &[Token]) -> Vec<Violation> {
    let home = agents_folder_of(path);
    let may_name_modules = REGISTRATION_FILES.contains(&path);
    let mut found = Vec::new();
    for integration in INTEGRATIONS.iter().filter(|i| home != Some(i.folder)) {
        for token in tokens {
            let what = match token.kind {
                TokenKind::Ident if token.text == integration.folder && !may_name_modules => {
                    format!("il modulo `{}`", integration.folder)
                }
                TokenKind::Str if token.text.contains(integration.id) => {
                    format!("la stringa {:?} (id di `{}`)", token.text, integration.folder)
                }
                TokenKind::Str if integration.commands.contains(&token.text.as_str()) => {
                    format!("il comando {:?} (di `{}`)", token.text, integration.folder)
                }
                _ => continue,
            };
            found.push(Violation { file: path.to_string(), line: token.line, what });
        }
    }
    found
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("cartella illeggibile {}: {e}", dir.display()));
    let mut paths: Vec<_> = entries.map(|e| e.expect("voce di cartella").path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// File che `file` dichiara come moduli di test (`#[cfg(test)] mod nome;`).
fn test_module_files(file: &Path, modules: &[String]) -> Vec<PathBuf> {
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let parent = file.parent().unwrap_or(Path::new(""));
    let base = if matches!(stem, "mod" | "lib" | "main") { parent.to_path_buf() } else { parent.join(stem) };
    modules.iter().flat_map(|m| [base.join(format!("{m}.rs")), base.join(m).join("mod.rs")]).collect()
}

fn relative(src: &Path, file: &Path) -> String {
    file.strip_prefix(src).unwrap_or(file).to_string_lossy().replace('\\', "/")
}

/// Violazioni di tutto il codice di produzione sotto `src`.
fn violations_under(src: &Path) -> Vec<Violation> {
    let mut files = Vec::new();
    rust_files(src, &mut files);
    let scans: Vec<_> = files
        .iter()
        .map(|file| {
            let source = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            (file, scan(&source))
        })
        .collect();
    let test_files: HashSet<PathBuf> =
        scans.iter().flat_map(|(file, scan)| test_module_files(file, &scan.test_modules)).collect();
    scans
        .iter()
        .filter(|(file, _)| !test_files.contains(*file))
        .flat_map(|(file, scan)| violations_in(&relative(src, file), &scan.tokens))
        .collect()
}

fn report(violations: &[Violation]) -> String {
    let lines: Vec<_> = violations.iter().map(|v| format!("  src/{}:{}: {}", v.file, v.line, v.what)).collect();
    format!(
        "Il core nomina un agente specifico (non sa come funziona un coding agent: offre dei contratti).\n{}\n\
         Sposta il dettaglio nell'integrazione, dietro un contratto. Guida: {WIKI_PAGE}",
        lines.join("\n")
    )
}

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

#[test]
fn the_core_does_not_name_a_specific_agent() {
    let found = violations_under(&src_dir());
    assert!(found.is_empty(), "{}", report(&found));
}

#[test]
fn every_integration_folder_is_known_and_registered() {
    let agents = src_dir().join("agents");
    let mut folders: Vec<String> = std::fs::read_dir(&agents)
        .expect("cartella agents")
        .map(|e| e.expect("voce di cartella").path())
        .filter(|p| p.is_dir())
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(str::to_string))
        .filter(|name| !CORE_FOLDERS.contains(&name.as_str()))
        .collect();
    folders.sort();
    let mut known: Vec<_> = INTEGRATIONS.iter().map(|i| i.folder.to_string()).collect();
    known.sort();
    assert_eq!(
        folders, known,
        "le cartelle di agents/ e INTEGRATIONS (architecture.rs) devono coincidere: {WIKI_PAGE}"
    );
    for integration in INTEGRATIONS {
        assert!(registry::find(integration.id).is_ok(), "{} non e' nel registro", integration.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn violations_of(path: &str, source: &str) -> Vec<Violation> {
        violations_in(path, &scan(source).tokens)
    }

    fn what(violations: &[Violation]) -> Vec<&str> {
        violations.iter().map(|v| v.what.as_str()).collect()
    }

    #[test]
    fn module_id_and_command_are_reported_outside_the_integration_folder() {
        let source = "use crate::agents::claude_code;\nfn f() { let a = \"claude-code\"; let b = \"claude.exe\"; }";
        let found = violations_of("workspaces/store.rs", source);
        assert_eq!(found.len(), 3, "{found:?}");
        assert_eq!(found.iter().map(|v| v.line).collect::<Vec<_>>(), vec![1, 2, 2]);
        assert!(what(&found)[1].contains("claude-code"));
    }

    #[test]
    fn the_integration_folder_may_name_itself() {
        let source = "const ID: &str = \"claude-code\"; const P: &str = \"claude\"; mod claude_code {}";
        assert!(violations_of("agents/claude_code/mod.rs", source).is_empty());
        assert!(violations_of("agents/claude_code/hook_events.rs", source).is_empty());
        // Il nome di una cartella non e' un prefisso: `agents/claude_codex/` non e' la sua.
        assert!(!violations_of("agents/claude_codex/mod.rs", source).is_empty());
        // Un file in `agents/` non e' dentro nessuna cartella di integrazione.
        assert!(!violations_of("agents/commands.rs", "const P: &str = \"claude\";").is_empty());
    }

    #[test]
    fn only_registration_files_may_name_the_module() {
        let module = "use super::claude_code; static I: &[u8] = &[claude_code::ClaudeCode];";
        assert!(violations_of("agents/registry.rs", module).is_empty());
        assert!(violations_of("agents/mod.rs", "mod claude_code;").is_empty());
        assert!(!violations_of("agents/package.rs", module).is_empty());
        // Le stringhe restano vietate anche nel registro.
        assert!(!violations_of("agents/registry.rs", "fn f() { find(\"claude-code\"); }").is_empty());
    }

    #[test]
    fn comments_tests_and_other_programs_are_not_reported() {
        let source = "\
            // usa claude-code e \"claude\"\n\
            /// il modulo claude_code\n\
            fn f() { let _ = \"claudette\"; let _ = \"claude code\"; let _ = \"builtin:preset\"; }\n\
            #[cfg(test)]\n\
            mod tests { fn t() { let _ = \"claude-code\"; let _ = \"claude\"; } }\n";
        assert!(violations_of("board.rs", source).is_empty());
    }

    #[test]
    fn ids_are_found_inside_longer_strings() {
        let found = violations_of("launch_settings.rs", "fn f() { let _ = \"builtin:claude-code\"; }");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn test_module_files_follow_the_module_layout() {
        let files = |file: &str| test_module_files(Path::new(file), &["tests".to_string()]);
        assert_eq!(
            files("src/agents/mod.rs"),
            vec![PathBuf::from("src/agents/tests.rs"), "src/agents/tests/mod.rs".into()]
        );
        assert_eq!(files("src/board.rs"), vec![PathBuf::from("src/board/tests.rs"), "src/board/tests/mod.rs".into()]);
    }

    #[test]
    fn the_report_points_to_the_wiki() {
        let message = report(&[Violation { file: "a.rs".into(), line: 3, what: "il modulo `x`".into() }]);
        assert!(message.contains("src/a.rs:3") && message.contains(WIKI_PAGE), "{message}");
    }
}
