//! I destinatari delle notifiche oltre al frontend: un `NoticeSink` registrato
//! (per esempio un connettore che le gira a un canale di contatto) riceve ogni
//! notifica raggruppata, con l'HUB acceso, anche se l'interruttore delle
//! notifiche di sistema e' spento: quell'interruttore riguarda solo il toast
//! del frontend. La decisione e' pura, cosi' si prova senza Tauri.

use super::notice::{AppMessage, ContactNotice, HubNotice};
use std::sync::Arc;

/// Chi riceve le notifiche raggruppate dell'HUB oltre al frontend. `deliver` e'
/// chiamato su un thread del pool bloccante, mai con un lock dell'HUB preso, e
/// puo' bloccare: per un lavoro lungo il sink si tiene una sua coda. Non deve
/// andare in panico; un errore va nel suo log.
pub trait NoticeSink: Send + Sync {
    fn deliver(&self, notice: &ContactNotice);

    /// L'utente ha scritto un messaggio nell'app (mai uno arrivato da un canale di
    /// contatto). A differenza di `deliver` e' chiamato dal percorso dell'invio, senza
    /// raggruppamento: deve essere veloce (mettere in coda e basta) e non andare in
    /// panico. Predefinita: lo ignora.
    fn app_message_written(&self, _message: &AppMessage) {}
}

/// A chi va una notifica, dato lo stato delle impostazioni e dei sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Targets {
    /// L'evento `hub-notify` verso il frontend: HUB acceso e notifiche accese.
    pub frontend: bool,
    /// I sink registrati: HUB acceso e almeno un sink.
    pub sinks: bool,
}

impl Targets {
    pub fn resolve(enabled: bool, notifications: bool, has_sinks: bool) -> Self {
        Self { frontend: enabled && notifications, sinks: enabled && has_sinks }
    }

    /// Serve raggruppare le notifiche solo se qualcuno le riceve.
    pub fn any(self) -> bool {
        self.frontend || self.sinks
    }
}

/// Consegna `notice` ai destinatari di `targets`: i sink uno dopo l'altro, poi
/// `emit` per il frontend.
pub fn dispatch(
    contact: &ContactNotice,
    targets: Targets,
    sinks: &[Arc<dyn NoticeSink>],
    emit: impl FnOnce(&HubNotice),
) {
    if targets.sinks {
        for sink in sinks {
            sink.deliver(contact);
        }
    }
    if targets.frontend {
        emit(&contact.notice);
    }
}

/// Consegna `message` ai sink, se `targets` li prevede.
pub fn dispatch_app_message(message: &AppMessage, targets: Targets, sinks: &[Arc<dyn NoticeSink>]) {
    if targets.sinks {
        for sink in sinks {
            sink.app_message_written(message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::notice::{AppMessage, HubNoticeSound, NoticeCategory};
    use super::*;
    use crate::hub::channel::Channel;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<ContactNotice>>, Mutex<Vec<AppMessage>>);

    impl NoticeSink for Recorder {
        fn deliver(&self, notice: &ContactNotice) {
            self.0.lock().unwrap().push(notice.clone());
        }

        fn app_message_written(&self, message: &AppMessage) {
            self.1.lock().unwrap().push(message.clone());
        }
    }

    fn app_message() -> AppMessage {
        AppMessage {
            channel: Channel::General,
            thread_id: "thr-1".into(),
            message_id: "msg-1".into(),
            channel_name: "Generale".into(),
            thread_title: "Piano".into(),
            text: "ciao".into(),
            attachment_count: 0,
        }
    }

    fn notice() -> ContactNotice {
        let notice = HubNotice {
            channel: Channel::General,
            thread_id: "thr-1".into(),
            message_id: "msg-1".into(),
            title: "t".into(),
            body: "b".into(),
            sound: HubNoticeSound::Ding,
        };
        ContactNotice {
            notice,
            category: NoticeCategory::TaskDone,
            channel_name: "Generale".into(),
            thread_title: "Piano".into(),
            text: "testo".into(),
        }
    }

    /// Esegue `dispatch` e ritorna (notifiche ai sink, emissioni al frontend).
    fn run(enabled: bool, notifications: bool, registered: bool) -> (usize, usize) {
        let recorder = Arc::new(Recorder::default());
        let sinks: Vec<Arc<dyn NoticeSink>> = if registered { vec![recorder.clone()] } else { Vec::new() };
        let targets = Targets::resolve(enabled, notifications, registered);
        let mut emitted = 0;
        dispatch(&notice(), targets, &sinks, |_| emitted += 1);
        let delivered = recorder.0.lock().unwrap().len();
        (delivered, emitted)
    }

    #[test]
    fn a_sink_receives_the_notice_even_with_the_notifications_switch_off() {
        assert_eq!(run(true, false, true), (1, 0));
    }

    #[test]
    fn the_frontend_is_emitted_only_with_the_switch_on() {
        assert_eq!(run(true, true, false), (0, 1));
        assert_eq!(run(true, true, true), (1, 1));
        assert_eq!(run(true, false, false), (0, 0));
    }

    #[test]
    fn nothing_goes_out_with_the_hub_off() {
        for notifications in [false, true] {
            assert_eq!(run(false, notifications, true), (0, 0));
        }
    }

    #[test]
    fn grouping_is_needed_only_when_someone_listens() {
        assert!(Targets::resolve(true, true, false).any());
        assert!(Targets::resolve(true, false, true).any());
        assert!(!Targets::resolve(true, false, false).any());
        assert!(!Targets::resolve(false, true, true).any());
    }

    #[test]
    fn every_registered_sink_is_called() {
        let (a, b) = (Arc::new(Recorder::default()), Arc::new(Recorder::default()));
        let sinks: Vec<Arc<dyn NoticeSink>> = vec![a.clone(), b.clone()];
        dispatch(&notice(), Targets::resolve(true, false, true), &sinks, |_| {});
        assert_eq!((a.0.lock().unwrap().len(), b.0.lock().unwrap().len()), (1, 1));
    }

    #[test]
    fn an_app_message_reaches_every_sink_with_the_hub_on_whatever_the_notifications_switch() {
        let (a, b) = (Arc::new(Recorder::default()), Arc::new(Recorder::default()));
        let sinks: Vec<Arc<dyn NoticeSink>> = vec![a.clone(), b.clone()];
        for notifications in [false, true] {
            dispatch_app_message(&app_message(), Targets::resolve(true, notifications, true), &sinks);
        }
        assert_eq!((a.1.lock().unwrap().len(), b.1.lock().unwrap().len()), (2, 2));
        assert!(a.0.lock().unwrap().is_empty(), "non e' una notifica");
    }

    #[test]
    fn an_app_message_goes_nowhere_with_the_hub_off_or_without_sinks() {
        let recorder = Arc::new(Recorder::default());
        let sinks: Vec<Arc<dyn NoticeSink>> = vec![recorder.clone()];
        dispatch_app_message(&app_message(), Targets::resolve(false, true, true), &sinks);
        dispatch_app_message(&app_message(), Targets::resolve(true, true, false), &sinks);
        assert!(recorder.1.lock().unwrap().is_empty());
    }
}
