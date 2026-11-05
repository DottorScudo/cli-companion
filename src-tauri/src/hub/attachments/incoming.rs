//! Gli allegati che arrivano come byte da una fonte esterna (una mail, una chat):
//! il piano di cosa tenere e cosa saltare (puro), la messa nello staging con gli
//! stessi limiti dell'app e la riga di nota sui file non importati. Un file che
//! non va bene non ferma il messaggio: si salta e lo si racconta. Bloccante:
//! va chiamato fuori dal lock dell'HUB.

use super::commands::staging_root;
use super::storage;
use super::{name, ATTACHMENTS_MAX, ATTACHMENT_SIZE_MAX};
use crate::hub::state::{HubState, ATTACHMENT_ID_PREFIX};
use crate::hub::text::clean_line;
use std::path::Path;
use tauri::{AppHandle, Manager};

/// Quanti nomi di file saltati si elencano nella nota (poi "e altri N").
const NOTE_NAMES_MAX: usize = 10;
/// Lunghezza di un nome nella nota, in caratteri.
const NOTE_NAME_MAX: usize = 60;
/// Il nome di un file che, ripulito, non ha caratteri.
const UNNAMED: &str = "(senza nome)";
const NOTE_PREFIX: &str = "Allegati non importati: ";

/// Un file ricevuto: il nome come l'ha dato la fonte (non fidato) e i suoi byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Perche' un file non e' stato importato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// Oltre `ATTACHMENTS_MAX` allegati.
    TooMany,
    /// Piu' di `ATTACHMENT_SIZE_MAX` byte.
    TooLarge,
    /// Il nome non si puo' usare come nome di file.
    InvalidName,
    /// Il salvataggio nello staging e' fallito.
    NotSaved,
}

impl SkipReason {
    fn label(self) -> &'static str {
        match self {
            Self::TooMany => "troppi allegati",
            Self::TooLarge => "troppo grande",
            Self::InvalidName => "nome non valido",
            Self::NotSaved => "non salvato",
        }
    }
}

/// Un file saltato: `name` e' gia' ripulito e tagliato, sicuro da mostrare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub name: String,
    pub reason: SkipReason,
}

/// L'esito: gli id degli allegati nello staging, in ordine d'arrivo, e i file saltati.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct StagedIncoming {
    pub ids: Vec<String>,
    pub skipped: Vec<Skipped>,
}

/// Decide per ogni file se tenerlo (`None`) o perche' saltarlo: i primi
/// `ATTACHMENTS_MAX` accettabili restano, gli altri sono `TooMany`. Puro.
pub fn plan(files: &[IncomingFile]) -> Vec<Option<SkipReason>> {
    let mut kept = 0;
    files
        .iter()
        .map(|file| {
            let reason = if file.bytes.len() as u64 > ATTACHMENT_SIZE_MAX {
                Some(SkipReason::TooLarge)
            } else if name::clean(&file.name).is_err() {
                Some(SkipReason::InvalidName)
            } else if kept >= ATTACHMENTS_MAX {
                Some(SkipReason::TooMany)
            } else {
                None
            };
            if reason.is_none() {
                kept += 1;
            }
            reason
        })
        .collect()
}

/// Mette nello staging dell'app i file ricevuti (vedi `stage_all`), con gli id
/// dell'`HubState`. Bloccante, fuori dal lock dell'HUB.
pub fn stage_incoming(app: &AppHandle, files: Vec<IncomingFile>) -> Result<StagedIncoming, String> {
    let root = staging_root(app)?;
    let state = app.state::<HubState>();
    let now = chrono::Utc::now().timestamp_millis();
    Ok(stage_all(&root, files, || state.next_id(ATTACHMENT_ID_PREFIX, now)))
}

/// Toglie dallo staging dell'app gli allegati `ids` che non servono piu'.
pub fn discard_staged_ids(app: &AppHandle, ids: &[String]) {
    match crate::hub::commands::hub_dirs_of(app) {
        Ok(dirs) => storage::discard_staged(&dirs.attachments, ids),
        Err(e) => log::error!("hub: allegati in staging non eliminati: {e}"),
    }
}

/// Mette nello staging di `root` i file tenuti da `plan`, con gli id di
/// `next_id`. Un errore di salvataggio salta quel file e lascia gli altri.
pub fn stage_all(root: &Path, files: Vec<IncomingFile>, mut next_id: impl FnMut() -> String) -> StagedIncoming {
    let verdicts = plan(&files);
    let mut out = StagedIncoming::default();
    for (file, verdict) in files.into_iter().zip(verdicts) {
        let reason = match verdict {
            Some(reason) => Some(reason),
            None => match storage::stage_bytes(root, &next_id(), &file.name, &file.bytes) {
                Ok(attachment) => {
                    out.ids.push(attachment.id);
                    None
                }
                Err(e) => {
                    log::warn!("hub: allegato in arrivo non importato: {e}");
                    Some(SkipReason::NotSaved)
                }
            },
        };
        if let Some(reason) = reason {
            out.skipped.push(Skipped { name: display_name(&file.name), reason });
        }
    }
    out
}

/// Il nome da mostrare di un file saltato: una riga, senza caratteri di controllo.
fn display_name(raw: &str) -> String {
    let name = clean_line(raw, NOTE_NAME_MAX);
    if name.is_empty() {
        UNNAMED.to_string()
    } else {
        name
    }
}

/// La riga di nota sui file saltati, `None` se non ce n'e':
/// `Allegati non importati: a.pdf (troppo grande), b.exe (nome non valido)`.
/// Elenca al massimo `NOTE_NAMES_MAX` file, poi "e altri N". Puro.
pub fn skipped_note(skipped: &[Skipped]) -> Option<String> {
    if skipped.is_empty() {
        return None;
    }
    let mut parts: Vec<String> =
        skipped.iter().take(NOTE_NAMES_MAX).map(|s| format!("{} ({})", s.name, s.reason.label())).collect();
    if skipped.len() > NOTE_NAMES_MAX {
        parts.push(format!("e altri {}", skipped.len() - NOTE_NAMES_MAX));
    }
    Some(format!("{NOTE_PREFIX}{}", parts.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn file(name: &str, len: usize) -> IncomingFile {
        IncomingFile { name: name.to_string(), bytes: vec![b'x'; len] }
    }

    fn temp_root(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-hub-incoming-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn counter() -> impl FnMut() -> String {
        let mut n = 0;
        move || {
            n += 1;
            format!("att-1-{n}")
        }
    }

    #[test]
    fn the_plan_keeps_the_first_files_within_the_limits() {
        let mut files: Vec<IncomingFile> = (0..ATTACHMENTS_MAX + 1).map(|i| file(&format!("f{i}.txt"), 1)).collect();
        files.push(file("grosso.bin", ATTACHMENT_SIZE_MAX as usize + 1));
        files.push(file("..", 1));
        let verdicts = plan(&files);
        assert!(verdicts[..ATTACHMENTS_MAX].iter().all(Option::is_none));
        assert_eq!(verdicts[ATTACHMENTS_MAX], Some(SkipReason::TooMany));
        assert_eq!(verdicts[ATTACHMENTS_MAX + 1], Some(SkipReason::TooLarge));
        assert_eq!(verdicts[ATTACHMENTS_MAX + 2], Some(SkipReason::InvalidName));
    }

    #[test]
    fn a_skipped_file_does_not_use_a_slot() {
        let mut files = vec![file("..", 1)];
        files.extend((0..ATTACHMENTS_MAX).map(|i| file(&format!("f{i}.txt"), 1)));
        assert!(plan(&files)[1..].iter().all(Option::is_none));
    }

    #[test]
    fn good_files_are_staged_and_bad_ones_reported() {
        let root = temp_root("stage");
        let files = vec![file("a.txt", 3), file("cartella/b.txt", 2), file("..", 1), file("", 1)];
        let out = stage_all(&root, files, counter());
        assert_eq!(out.ids, ["att-1-1", "att-1-2"]);
        assert!(root.join("staging").join("att-1-1").join("a.txt").is_file());
        assert!(root.join("staging").join("att-1-2").join("b.txt").is_file());
        assert_eq!(out.skipped.len(), 2);
        assert_eq!(out.skipped[0].name, "..");
        assert_eq!(out.skipped[1], Skipped { name: UNNAMED.to_string(), reason: SkipReason::InvalidName });
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_oversized_file_is_never_written() {
        let root = temp_root("big");
        let out = stage_all(&root, vec![file("grosso.bin", ATTACHMENT_SIZE_MAX as usize + 1)], counter());
        assert!(out.ids.is_empty());
        assert_eq!(out.skipped, [Skipped { name: "grosso.bin".to_string(), reason: SkipReason::TooLarge }]);
        assert!(!root.exists());
    }

    #[test]
    fn a_failed_save_skips_the_file_and_keeps_the_others() {
        let root = temp_root("fail");
        // Lo stesso id due volte: il secondo salvataggio trova gia' il file.
        let out = stage_all(&root, vec![file("a.txt", 1), file("a.txt", 1)], || "att-1-1".to_string());
        assert_eq!(out.ids, ["att-1-1"]);
        assert_eq!(out.skipped, [Skipped { name: "a.txt".to_string(), reason: SkipReason::NotSaved }]);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_note_lists_names_with_italian_reasons() {
        assert_eq!(skipped_note(&[]), None);
        let skipped = [
            Skipped { name: "a.pdf".into(), reason: SkipReason::TooLarge },
            Skipped { name: "b.exe".into(), reason: SkipReason::InvalidName },
        ];
        assert_eq!(
            skipped_note(&skipped).unwrap(),
            "Allegati non importati: a.pdf (troppo grande), b.exe (nome non valido)"
        );
    }

    #[test]
    fn the_note_is_bounded_and_names_are_clean_single_lines() {
        let many: Vec<Skipped> =
            (0..NOTE_NAMES_MAX + 3).map(|i| Skipped { name: format!("f{i}"), reason: SkipReason::TooMany }).collect();
        assert!(skipped_note(&many).unwrap().ends_with(", e altri 3"));
        let long = display_name(&format!("a\nb\u{1b}{}", "x".repeat(200)));
        assert!(!long.chars().any(char::is_control) && long.chars().count() <= NOTE_NAME_MAX + 1);
    }
}
