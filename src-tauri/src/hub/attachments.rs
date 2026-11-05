//! Gli allegati dei messaggi dell'utente. Nel thread (e quindi nel JSON del
//! canale) ci sono solo i metadati (`model::Attachment`); i file stanno in
//! `<app_data>/hub_attachments/` (`layout`), mai nella repository dell'utente.
//!
//! Caricamento in due fasi: il frontend mette il file nello staging (da un
//! percorso o dai byte) e riceve i metadati con l'id; `hub_send` riceve gli id e
//! sposta i file nella cartella del thread prima di salvare il messaggio (se il
//! salvataggio fallisce tornano nello staging). I file di messaggi e thread
//! potati, e di un canale dimenticato, si cancellano dopo il salvataggio
//! riuscito (`garbage`, puro, e `storage::discard`).
//!
//! Un file per responsabilita':
//! - `name`: pulizia dei nomi per Windows (puro);
//! - `mime`: il tipo dedotto da contenuto ed estensione (puro);
//! - `layout`: i percorsi, con la validazione di ogni componente (puro);
//! - `garbage`: quali file cancellare dopo un salvataggio (puro);
//! - `listing`: come gli allegati si raccontano a modelli e agenti (puro);
//! - `storage`: le operazioni su disco (staging, spostamento, cancellazione);
//! - `commands`: i comandi Tauri e il percorso assoluto di un allegato.

pub mod commands;
mod garbage;
mod layout;
pub mod listing;
mod mime;
mod name;
mod storage;

pub use garbage::{garbage, Garbage};
pub use layout::stored_file;
pub use storage::{discard, forget_channel, Claimed, NOT_FOUND};

use super::model::Attachment;
use super::text::valid_ref_id;
use std::collections::HashSet;

/// Allegati di un messaggio.
pub const ATTACHMENTS_MAX: usize = 5;
/// Dimensione di un allegato, in byte (10 MB).
pub const ATTACHMENT_SIZE_MAX: u64 = 10 * 1024 * 1024;

/// Gli id scelti dal frontend per un messaggio: al massimo `ATTACHMENTS_MAX`,
/// senza doppioni, ognuno nella forma di un id dell'app.
pub fn validate_ids(ids: &[String]) -> Result<(), String> {
    if ids.len() > ATTACHMENTS_MAX {
        return Err(too_many());
    }
    let mut seen = HashSet::new();
    for id in ids {
        if !valid_ref_id(id) {
            return Err("id dell'allegato non valido".to_string());
        }
        if !seen.insert(id.as_str()) {
            return Err("lo stesso allegato è indicato due volte".to_string());
        }
    }
    Ok(())
}

/// I metadati degli allegati di un messaggio: limiti, id unici e validi, nomi
/// gia' ripuliti. Li costruisce l'app, ma un file dei thread modificato a mano
/// non deve poter indicare un percorso fuori dalla cartella degli allegati.
pub fn validate(list: &[Attachment]) -> Result<(), String> {
    let ids: Vec<String> = list.iter().map(|a| a.id.clone()).collect();
    validate_ids(&ids)?;
    for attachment in list {
        if attachment.size > ATTACHMENT_SIZE_MAX {
            return Err(too_large(&attachment.name));
        }
        if name::clean(&attachment.name).as_deref() != Ok(attachment.name.as_str()) {
            return Err("nome dell'allegato non valido".to_string());
        }
    }
    Ok(())
}

fn too_many() -> String {
    format!("un messaggio può avere al massimo {ATTACHMENTS_MAX} allegati")
}

fn too_large(name: &str) -> String {
    format!("«{name}» supera i {} MB consentiti per un allegato", ATTACHMENT_SIZE_MAX / (1024 * 1024))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attachment(id: &str, name: &str, size: u64) -> Attachment {
        Attachment { id: id.to_string(), name: name.to_string(), mime: "text/plain".to_string(), size }
    }

    #[test]
    fn ids_are_bounded_unique_and_safe() {
        let ids = |n: usize| (0..n).map(|i| format!("att-1-{i}")).collect::<Vec<_>>();
        assert!(validate_ids(&[]).is_ok() && validate_ids(&ids(ATTACHMENTS_MAX)).is_ok());
        assert!(validate_ids(&ids(ATTACHMENTS_MAX + 1)).unwrap_err().contains("5"));
        assert!(validate_ids(&["att-1-1".to_string(), "att-1-1".to_string()]).unwrap_err().contains("due volte"));
        for bad in ["", "..", "../x", "a/b", "a\\b", "a.b"] {
            assert!(validate_ids(&[bad.to_string()]).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn metadata_must_respect_the_limits_and_carry_clean_names() {
        assert!(validate(&[attachment("att-1-1", "nota.txt", ATTACHMENT_SIZE_MAX)]).is_ok());
        assert!(validate(&[attachment("att-1-1", "nota.txt", ATTACHMENT_SIZE_MAX + 1)]).unwrap_err().contains("10 MB"));
        for bad in ["../x.txt", "a/b.txt", "CON.txt", "x.", ""] {
            assert!(validate(&[attachment("att-1-1", bad, 1)]).is_err(), "{bad:?}");
        }
    }
}
