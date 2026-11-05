//! La colla con Tauri: controlla le impostazioni, costruisce la notifica, la
//! tiene per la finestra di raggruppamento e poi emette `hub-notify`. Nessun
//! lock resta preso durante l'attesa o l'emissione.

use super::kind::NoticeKind;
use super::notice::{HubNotice, HUB_NOTIFY_EVENT};
use super::pending::{Candidate, Pending, ThreadKey};
use super::text;
use crate::hub::channel::Channel;
use crate::hub::model::ThreadSummary;
use crate::hub::settings::HubSettingsStore;
use crate::workspaces::WorkspaceStore;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Quanto si aspetta, dal primo candidato di un thread, prima di emettere:
/// un evento seguito dalla risposta del titolare diventa una notifica sola.
const GROUP_WINDOW: Duration = Duration::from_secs(3);

/// Stato Tauri: i candidati in attesa della fine della loro finestra.
#[derive(Default)]
pub struct HubNotifier {
    pending: Mutex<Pending>,
}

impl HubNotifier {
    /// Un candidato per il thread `summary`: `message_text` e' il messaggio che
    /// l'ha causato. Non fa nulla con l'HUB o le notifiche spenti. Veloce: la
    /// finestra e l'emissione girano su un thread a parte.
    pub fn notify(&self, app: &AppHandle, summary: &ThreadSummary, kind: NoticeKind, message_text: &str) {
        if !notifications_on(app) {
            return;
        }
        let workspace_name = match &summary.channel {
            Channel::General => None,
            Channel::Workspace(id) => app.state::<WorkspaceStore>().find(id).map(|w| w.name),
        };
        let key = (summary.channel.clone(), summary.id.clone());
        let candidate = candidate(summary, &kind, message_text, workspace_name.as_deref());
        if !self.lock().offer(key.clone(), candidate) {
            return;
        }
        let app = app.clone();
        // Il progetto non ha un timer async: un thread del pool bloccante che
        // dorme per la finestra.
        tauri::async_runtime::spawn_blocking(move || {
            std::thread::sleep(GROUP_WINDOW);
            app.state::<HubNotifier>().flush(&app, &key);
        });
    }

    /// L'HUB e' stato spento: nessuna notifica in attesa deve uscire.
    pub fn discard(&self) {
        self.lock().clear();
    }

    /// Chiude la finestra di `key` ed emette quanto resta, se le notifiche
    /// sono ancora accese.
    fn flush(&self, app: &AppHandle, key: &ThreadKey) {
        let notice = self.lock().take(key);
        let Some(notice) = notice.filter(|_| notifications_on(app)) else {
            return;
        };
        if let Err(e) = app.emit(HUB_NOTIFY_EVENT, notice) {
            log::warn!("hub: notifica non emessa: {e}");
        }
    }

    fn lock(&self) -> MutexGuard<'_, Pending> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn notifications_on(app: &AppHandle) -> bool {
    app.state::<HubSettingsStore>().read(|s| s.enabled && s.notifications)
}

fn candidate(
    summary: &ThreadSummary,
    kind: &NoticeKind,
    message_text: &str,
    workspace_name: Option<&str>,
) -> Candidate {
    let channel = text::channel_name(&summary.channel, workspace_name);
    Candidate {
        priority: kind.priority(),
        notice: HubNotice {
            channel: summary.channel.clone(),
            thread_id: summary.id.clone(),
            title: kind.title(),
            body: text::body(&channel, &summary.title, message_text),
            sound: kind.sound(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::super::kind::Priority;
    use super::super::notice::HubNoticeSound;
    use super::*;
    use crate::hub::model::ThreadState;

    fn summary(channel: Channel) -> ThreadSummary {
        ThreadSummary {
            id: "thr-1-1".into(),
            channel,
            title: "Migrazione".into(),
            state: ThreadState::Open,
            task_count: 0,
            message_count: 1,
            waiting_user: false,
            owner: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn a_reply_candidate_carries_title_sound_and_names_instead_of_ids() {
        let s = summary(Channel::Workspace("ws-9-9".into()));
        let c = candidate(&s, &NoticeKind::Reply { bot: "ada".into() }, "Fatto.\nAltro", Some("Sito"));
        assert_eq!(c.priority, Priority::Reply);
        assert_eq!(c.notice.title, "@ada ti ha risposto");
        assert_eq!(c.notice.body, "Sito · Migrazione\nFatto.");
        assert_eq!(c.notice.sound, HubNoticeSound::Ding);
        assert_eq!((c.notice.channel, c.notice.thread_id.as_str()), (Channel::Workspace("ws-9-9".into()), "thr-1-1"));
    }

    #[test]
    fn the_general_channel_and_a_removed_workspace_have_fixed_names() {
        let general = candidate(&summary(Channel::General), &NoticeKind::TaskFailed, "", None);
        assert_eq!(general.notice.body, "Generale · Migrazione");
        assert_eq!(general.notice.sound, HubNoticeSound::Attention);
        let gone = candidate(&summary(Channel::Workspace("ws-9-9".into())), &NoticeKind::TaskDone, "ok", None);
        assert_eq!(gone.notice.body, "Workspace · Migrazione\nok");
        assert!(!gone.notice.body.contains("ws-9-9"));
    }

    #[test]
    fn discarding_drops_the_pending_notices() {
        let notifier = HubNotifier::default();
        let s = summary(Channel::General);
        let key = (s.channel.clone(), s.id.clone());
        notifier.lock().offer(key.clone(), candidate(&s, &NoticeKind::TaskDone, "x", None));
        notifier.discard();
        assert!(notifier.lock().take(&key).is_none());
    }
}
