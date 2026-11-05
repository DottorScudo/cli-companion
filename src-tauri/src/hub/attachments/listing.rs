//! Come gli allegati si raccontano ai modelli e agli agenti (puro, in inglese):
//! una riga di metadati per i segretari, che non vedono mai il contenuto ne' il
//! percorso, e una riga con il percorso assoluto per pianificatore e worker,
//! che possono leggere il file, nella forma del loro ambiente (`/mnt/<d>/...`
//! in un workspace WSL). Mai il contenuto.

use super::stored_file;
use crate::hub::channel::Channel;
use crate::hub::model::Attachment;
use crate::platform::{windows_to_wsl_path, TargetKind};
use std::path::Path;

const KIB: u64 = 1024;
const MIB: u64 = KIB * 1024;

/// Dove stanno gli allegati sull'host (`HubDirs::attachments`) e l'ambiente
/// dell'agente che li legge, da cui dipende la forma dei percorsi.
#[derive(Debug, Clone, Copy)]
pub struct AttachmentPaths<'a> {
    pub root: &'a Path,
    pub env: TargetKind,
}

/// Nome, tipo e dimensione: `report.pdf (application/pdf, 1.5 KB)`.
pub fn describe(attachment: &Attachment) -> String {
    format!("{} ({}, {})", attachment.name, attachment.mime, size_text(attachment.size))
}

/// Come `describe`, con davanti il percorso assoluto come lo vede l'agente. Se
/// il percorso non si ricava (metadati modificati a mano, percorso che WSL non
/// raggiunge) lo dice, senza inventarne uno.
pub fn describe_with_path(
    paths: AttachmentPaths<'_>,
    channel: &Channel,
    thread_id: &str,
    attachment: &Attachment,
) -> String {
    let path = stored_file(paths.root, channel, thread_id, attachment).and_then(|file| path_for_env(&file, paths.env));
    match path {
        Ok(path) => format!("{path} — {}", describe(attachment)),
        Err(e) => {
            log::warn!("hub: percorso dell'allegato {} non ricavato: {e}", attachment.id);
            format!("(path unavailable) — {}", describe(attachment))
        }
    }
}

/// Il percorso nell'ambiente dell'agente: invariato su Windows, `/mnt/<d>/...` in WSL.
fn path_for_env(file: &Path, env: TargetKind) -> Result<String, String> {
    let host = file.display().to_string();
    match env {
        TargetKind::Windows => Ok(host),
        TargetKind::Wsl => {
            windows_to_wsl_path(&host).ok_or_else(|| format!("percorso non raggiungibile da WSL: {host}"))
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

    const HOST_ROOT: &str = r"C:\Users\Me\AppData\Roaming\cli-companion\hub_attachments";

    fn attachment(name: &str, size: u64) -> Attachment {
        Attachment { id: "att-1-1".to_string(), name: name.to_string(), mime: "application/pdf".to_string(), size }
    }

    fn on(root: &Path, env: TargetKind) -> AttachmentPaths<'_> {
        AttachmentPaths { root, env }
    }

    fn ws() -> Channel {
        Channel::Workspace("ws-1".to_string())
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
        let line =
            describe_with_path(on(&root, TargetKind::Windows), &ws(), "thr-1-1", &attachment("report.pdf", 1536));
        let expected = root.join("channels").join("ws-1").join("thr-1-1").join("att-1-1").join("report.pdf");
        assert_eq!(line, format!("{} — report.pdf (application/pdf, 1.5 KB)", expected.display()));
    }

    #[cfg(windows)]
    #[test]
    fn a_windows_agent_reads_the_windows_path() {
        let line = describe_with_path(
            on(Path::new(HOST_ROOT), TargetKind::Windows),
            &ws(),
            "thr-1-1",
            &attachment("a.pdf", 1),
        );
        assert_eq!(
            line,
            format!(r"{HOST_ROOT}\channels\ws-1\thr-1-1\att-1-1\a.pdf — a.pdf (application/pdf, 1 bytes)")
        );
    }

    #[test]
    fn a_wsl_agent_reads_the_mounted_drive_path() {
        let line =
            describe_with_path(on(Path::new(HOST_ROOT), TargetKind::Wsl), &ws(), "thr-1-1", &attachment("a.pdf", 1));
        assert_eq!(
            line,
            "/mnt/c/Users/Me/AppData/Roaming/cli-companion/hub_attachments/channels/ws-1/thr-1-1/att-1-1/a.pdf — \
             a.pdf (application/pdf, 1 bytes)"
        );
    }

    #[test]
    fn a_path_wsl_cannot_reach_is_not_invented() {
        let line =
            describe_with_path(on(Path::new("data"), TargetKind::Wsl), &ws(), "thr-1-1", &attachment("a.pdf", 1));
        assert_eq!(line, "(path unavailable) — a.pdf (application/pdf, 1 bytes)");
    }

    #[test]
    fn an_unsafe_attachment_gets_no_path() {
        let root = Path::new("root");
        let line =
            describe_with_path(on(root, TargetKind::Windows), &Channel::General, "thr-1-1", &attachment("../x.pdf", 1));
        assert_eq!(line, "(path unavailable) — ../x.pdf (application/pdf, 1 bytes)");
    }
}
