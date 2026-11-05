//! Gli allegati di un messaggio in entrata: si scaricano solo per un account
//! collegato, al massimo `ATTACHMENTS_MAX`, saltando quelli piu' grandi del
//! limite (dichiarato dal servizio o, dopo lo scarico, reale). Cio' che non si
//! importa si dice all'utente una volta sola.

use super::super::replies::{SkipReason, SkippedFile};
use super::super::ContactCtx;
use crate::connectors::contracts::{ConnectorError, RemoteFile};
use crate::connectors::limits::CALL_TIMEOUT;
use crate::connectors::runtime::time::with_timeout;
use crate::hub::attachments::{IncomingFile, ATTACHMENTS_MAX, ATTACHMENT_SIZE_MAX};

fn skipped(file: &RemoteFile, reason: SkipReason) -> SkippedFile {
    SkippedFile { name: file.name.clone(), reason }
}

fn too_big(bytes: u64) -> bool {
    bytes > ATTACHMENT_SIZE_MAX
}

/// Quali allegati scaricare: i primi `ATTACHMENTS_MAX` la cui dimensione dichiarata
/// non supera il limite; gli altri sono scartati con il motivo. Pura.
fn plan(remote: &[RemoteFile]) -> (Vec<&RemoteFile>, Vec<SkippedFile>) {
    let mut to_fetch = Vec::new();
    let mut left_out = Vec::new();
    for (index, file) in remote.iter().enumerate() {
        if index >= ATTACHMENTS_MAX {
            left_out.push(skipped(file, SkipReason::TooMany));
        } else if file.size.is_some_and(too_big) {
            left_out.push(skipped(file, SkipReason::TooBig));
        } else {
            to_fetch.push(file);
        }
    }
    (to_fetch, left_out)
}

impl ContactCtx<'_> {
    /// Scarica gli allegati ammessi. Un errore che ferma il supervisore risale; un altro
    /// fa saltare quel solo file (registrato con la classe).
    pub(super) async fn collect_files(
        &self,
        remote: &[RemoteFile],
    ) -> Result<(Vec<IncomingFile>, Vec<SkippedFile>), ConnectorError> {
        let (to_fetch, mut left_out) = plan(remote);
        let mut files = Vec::new();
        for file in to_fetch {
            match with_timeout(CALL_TIMEOUT, self.connector.fetch_file(self.ctx, file)).await {
                Ok(fetched) if too_big(fetched.bytes.len() as u64) => left_out.push(skipped(file, SkipReason::TooBig)),
                Ok(fetched) => files.push(fetched),
                Err(e) if e.stops_the_supervisor() => return Err(e),
                Err(e) => {
                    log::warn!("connettori {}: allegato non scaricato ({})", self.instance(), e.class());
                    left_out.push(skipped(file, SkipReason::NotDownloaded));
                }
            }
        }
        Ok((files, left_out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(name: &str, size: Option<u64>) -> RemoteFile {
        RemoteFile { id: format!("id-{name}"), name: name.to_string(), size }
    }

    #[test]
    fn files_within_the_limits_are_all_fetched() {
        let files = vec![remote("a", Some(10)), remote("b", None), remote("c", Some(ATTACHMENT_SIZE_MAX))];
        let (fetch, left_out) = plan(&files);
        assert_eq!(fetch.len(), 3);
        assert!(left_out.is_empty());
    }

    #[test]
    fn a_declared_size_over_the_limit_is_left_out() {
        let files = vec![remote("big", Some(ATTACHMENT_SIZE_MAX + 1)), remote("ok", Some(1))];
        let (fetch, left_out) = plan(&files);
        assert_eq!(fetch.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["ok"]);
        assert_eq!(left_out, vec![SkippedFile { name: "big".to_string(), reason: SkipReason::TooBig }]);
    }

    #[test]
    fn only_the_first_files_up_to_the_cap_are_considered() {
        let files: Vec<RemoteFile> = (0..ATTACHMENTS_MAX + 2).map(|i| remote(&format!("f{i}"), Some(1))).collect();
        let (fetch, left_out) = plan(&files);
        assert_eq!(fetch.len(), ATTACHMENTS_MAX);
        let names: Vec<&str> = left_out.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, [format!("f{ATTACHMENTS_MAX}"), format!("f{}", ATTACHMENTS_MAX + 1)]);
        assert!(left_out.iter().all(|f| f.reason == SkipReason::TooMany));
    }

    #[test]
    fn oversized_files_inside_the_cap_still_use_a_slot() {
        let mut files: Vec<RemoteFile> = (0..ATTACHMENTS_MAX).map(|i| remote(&format!("f{i}"), Some(1))).collect();
        files[0].size = Some(ATTACHMENT_SIZE_MAX + 1);
        files.push(remote("extra", Some(1)));
        let (fetch, left_out) = plan(&files);
        assert_eq!(fetch.len(), ATTACHMENTS_MAX - 1);
        assert_eq!(left_out.len(), 2);
    }
}
