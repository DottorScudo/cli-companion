//! I percorsi degli allegati, sotto la radice `<app_data>/hub_attachments/`:
//!
//! - `staging/<id>/<nome>`: caricato, non ancora inviato;
//! - `channels/<canale>/<thread_id>/<id>/<nome>`: allegato a un messaggio.
//!
//! Una cartella per allegato: il nome resta quello ripulito (fino a 120
//! caratteri, sotto il limite di 255 di un componente) e si cancella o sposta
//! l'allegato intero con la sua cartella. Ogni componente si valida qui
//! (`valid_ref_id` per id e thread, `name::clean` per il nome, il canale e' gia'
//! un nome di file sicuro): nessun percorso puo' uscire dalla radice. Puro.

use super::name;
use crate::hub::channel::Channel;
use crate::hub::model::Attachment;
use crate::hub::text::valid_ref_id;
use std::path::{Path, PathBuf};

const STAGING_DIR: &str = "staging";
const CHANNELS_DIR: &str = "channels";

pub const INVALID_ID: &str = "id dell'allegato non valido";
const INVALID_THREAD: &str = "id del thread non valido";

pub fn staging_dir(root: &Path) -> PathBuf {
    root.join(STAGING_DIR)
}

/// La cartella di un allegato nello staging.
pub fn staged_dir(root: &Path, id: &str) -> Result<PathBuf, String> {
    Ok(staging_dir(root).join(checked(id, INVALID_ID)?))
}

pub fn channel_dir(root: &Path, channel: &Channel) -> PathBuf {
    root.join(CHANNELS_DIR).join(channel.as_str())
}

pub fn thread_dir(root: &Path, channel: &Channel, thread_id: &str) -> Result<PathBuf, String> {
    Ok(channel_dir(root, channel).join(checked(thread_id, INVALID_THREAD)?))
}

/// La cartella di un allegato inviato nel thread.
pub fn stored_dir(root: &Path, channel: &Channel, thread_id: &str, id: &str) -> Result<PathBuf, String> {
    Ok(thread_dir(root, channel, thread_id)?.join(checked(id, INVALID_ID)?))
}

/// Il file di un allegato inviato nel thread. Il nome dei metadati deve essere
/// gia' ripulito: uno modificato a mano nel file dei thread e' rifiutato.
pub fn stored_file(
    root: &Path,
    channel: &Channel,
    thread_id: &str,
    attachment: &Attachment,
) -> Result<PathBuf, String> {
    if name::clean(&attachment.name).as_deref() != Ok(attachment.name.as_str()) {
        return Err("nome dell'allegato non valido".to_string());
    }
    Ok(stored_dir(root, channel, thread_id, &attachment.id)?.join(&attachment.name))
}

fn checked<'a>(component: &'a str, error: &str) -> Result<&'a str, String> {
    if valid_ref_id(component) {
        Ok(component)
    } else {
        Err(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attachment(id: &str, name: &str) -> Attachment {
        Attachment { id: id.to_string(), name: name.to_string(), mime: "text/plain".to_string(), size: 1 }
    }

    fn ws() -> Channel {
        Channel::Workspace("ws-1".to_string())
    }

    #[test]
    fn paths_follow_the_documented_layout() {
        let root = Path::new("data").join("hub_attachments");
        assert_eq!(staged_dir(&root, "att-1-2").unwrap(), root.join("staging").join("att-1-2"));
        let file = stored_file(&root, &ws(), "thr-1-1", &attachment("att-1-2", "a b.txt")).unwrap();
        assert_eq!(file, root.join("channels").join("ws-1").join("thr-1-1").join("att-1-2").join("a b.txt"));
        assert_eq!(channel_dir(&root, &Channel::General), root.join("channels").join("general"));
    }

    #[test]
    fn no_component_can_escape_the_root() {
        let root = Path::new("root");
        for bad in ["", ".", "..", "../x", "a/b", "a\\b", "C:", "a.b", "é"] {
            assert!(staged_dir(root, bad).is_err(), "{bad:?}");
            assert!(thread_dir(root, &ws(), bad).is_err(), "{bad:?}");
            assert!(stored_dir(root, &ws(), "thr-1", bad).is_err(), "{bad:?}");
            assert!(stored_file(root, &ws(), bad, &attachment("att-1", "x.txt")).is_err(), "{bad:?}");
        }
        for bad in ["..", "../x.txt", "a/b.txt", "a\\b.txt", "C:\\x.txt", "CON", "x.", ""] {
            assert!(stored_file(root, &ws(), "thr-1", &attachment("att-1", bad)).is_err(), "{bad:?}");
        }
    }
}
