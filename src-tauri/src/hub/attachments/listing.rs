//! Come gli allegati si raccontano ai modelli e agli agenti (puro, in inglese):
//! una riga di metadati per i segretari, che non vedono mai il contenuto ne' il
//! percorso, e una riga con il percorso assoluto per pianificatore e worker,
//! che possono leggere il file. Mai il contenuto.

use super::stored_file;
use crate::hub::channel::Channel;
use crate::hub::model::Attachment;
use std::path::Path;

const KIB: u64 = 1024;
const MIB: u64 = KIB * 1024;

/// Nome, tipo e dimensione: `report.pdf (application/pdf, 1.5 KB)`.
pub fn describe(attachment: &Attachment) -> String {
    format!("{} ({}, {})", attachment.name, attachment.mime, size_text(attachment.size))
}

/// Come `describe`, con il percorso assoluto davanti. Se il percorso non si
/// ricava (metadati modificati a mano) lo dice, senza inventarne uno.
pub fn describe_with_path(root: &Path, channel: &Channel, thread_id: &str, attachment: &Attachment) -> String {
    match stored_file(root, channel, thread_id, attachment) {
        Ok(path) => format!("{} — {}", path.display(), describe(attachment)),
        Err(e) => {
            log::warn!("hub: percorso dell'allegato {} non ricavato: {e}", attachment.id);
            format!("(path unavailable) — {}", describe(attachment))
        }
    }
}

/// La dimensione leggibile: byte, KB o MB con un decimale.
pub fn size_text(bytes: u64) -> String {
    match bytes {
        0..KIB => format!("{bytes} bytes"),
        KIB..MIB => format!("{:.1} KB", bytes as f64 / KIB as f64),
        _ => format!("{:.1} MB", bytes as f64 / MIB as f64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attachment(name: &str, size: u64) -> Attachment {
        Attachment { id: "att-1-1".to_string(), name: name.to_string(), mime: "application/pdf".to_string(), size }
    }

    #[test]
    fn sizes_are_readable() {
        assert_eq!(size_text(0), "0 bytes");
        assert_eq!(size_text(1023), "1023 bytes");
        assert_eq!(size_text(1536), "1.5 KB");
        assert_eq!(size_text(10 * MIB), "10.0 MB");
    }

    #[test]
    fn the_description_has_name_type_and_size_but_no_path() {
        assert_eq!(describe(&attachment("report.pdf", 1536)), "report.pdf (application/pdf, 1.5 KB)");
    }

    #[test]
    fn the_path_comes_from_the_storage_layout() {
        let root = Path::new("data").join("hub_attachments");
        let channel = Channel::Workspace("ws-1".to_string());
        let line = describe_with_path(&root, &channel, "thr-1-1", &attachment("report.pdf", 1536));
        let expected = root.join("channels").join("ws-1").join("thr-1-1").join("att-1-1").join("report.pdf");
        assert_eq!(line, format!("{} — report.pdf (application/pdf, 1.5 KB)", expected.display()));
    }

    #[test]
    fn an_unsafe_attachment_gets_no_path() {
        let line = describe_with_path(Path::new("root"), &Channel::General, "thr-1-1", &attachment("../x.pdf", 1));
        assert_eq!(line, "(path unavailable) — ../x.pdf (application/pdf, 1 bytes)");
    }
}
