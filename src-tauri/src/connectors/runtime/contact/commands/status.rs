//! `/status`, `/mute [minuti]` e `/unmute`.

use super::super::replies::{self, MuteState};
use super::super::{ContactCtx, Place};
use crate::connectors::contracts::ConnectorError;
use crate::connectors::model::ConnectorLink;
use crate::hub::contact::ThreadEntry;

/// Minuti al massimo di un silenzio con durata (7 giorni).
const MUTE_MINUTES_MAX: u64 = 7 * 24 * 60;
const MS_PER_MINUTE: u64 = 60_000;

/// Cosa chiede `/mute`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MuteRequest {
    UntilUnmute,
    Minutes(u64),
}

/// `/mute` senza argomento silenzia a oltranza; con un numero di minuti da 1 a
/// `MUTE_MINUTES_MAX`, per quel tempo; altro non vale. Pura.
fn parse_mute(arg: &str) -> Option<MuteRequest> {
    let arg = arg.trim();
    if arg.is_empty() {
        return Some(MuteRequest::UntilUnmute);
    }
    arg.parse::<u64>().ok().filter(|m| (1..=MUTE_MINUTES_MAX).contains(m)).map(MuteRequest::Minutes)
}

/// Lo stato del silenzio all'istante `now`. Pura.
fn mute_state(muted_until: Option<i64>, now: i64) -> MuteState {
    match muted_until {
        Some(until) if until == i64::MAX => MuteState::Forever,
        Some(until) if until > now => {
            let left = u64::try_from(until.saturating_sub(now)).unwrap_or(1);
            MuteState::Minutes(left.div_ceil(MS_PER_MINUTE).max(1))
        }
        _ => MuteState::Off,
    }
}

/// Quanti thread aspettano l'utente, per nome di canale (nell'ordine in cui compaiono). Pura.
fn waiting_by_channel(entries: &[ThreadEntry]) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for entry in entries.iter().filter(|e| e.waiting_user) {
        match counts.iter_mut().find(|(name, _)| *name == entry.channel_name) {
            Some((_, n)) => *n += 1,
            None => counts.push((entry.channel_name.clone(), 1)),
        }
    }
    counts
}

/// Come chiamare il thread corrente del collegamento. Pura.
fn current_label(link: &ConnectorLink, open: &[ThreadEntry]) -> String {
    let Some(current) = &link.current else { return replies::no_current_label() };
    match open.iter().find(|e| e.channel == current.channel && e.thread_id == current.thread_id) {
        Some(entry) => replies::thread_label(&entry.channel_name, &entry.title),
        None => replies::current_no_longer_open(),
    }
}

impl ContactCtx<'_> {
    pub(super) async fn show_status(&self, link: &ConnectorLink, place: &Place) -> Result<(), ConnectorError> {
        let open = match self.hub.open_threads().await {
            Ok(open) => open,
            Err(e) => return self.reply(place, &replies::hub_error(&e)).await,
        };
        let mute = mute_state(link.muted_until, self.clock.now_ms());
        let text = replies::status(&current_label(link, &open), &waiting_by_channel(&open), mute);
        self.reply(place, &text).await
    }

    pub(super) async fn mute(&self, link: &ConnectorLink, place: &Place, arg: &str) -> Result<(), ConnectorError> {
        let Some(request) = parse_mute(arg) else {
            return self.reply(place, &replies::mute_usage(MUTE_MINUTES_MAX)).await;
        };
        let (until, done) = match request {
            MuteRequest::UntilUnmute => (i64::MAX, replies::muted_forever()),
            MuteRequest::Minutes(minutes) => {
                let span = i64::try_from(minutes.saturating_mul(MS_PER_MINUTE)).unwrap_or(i64::MAX);
                (self.clock.now_ms().saturating_add(span), replies::muted_for(minutes))
            }
        };
        let text = if self.change_link(&link.id, move |l| l.muted_until = Some(until)).await {
            done
        } else {
            replies::not_saved()
        };
        self.reply(place, &text).await
    }

    pub(super) async fn unmute(&self, link: &ConnectorLink, place: &Place) -> Result<(), ConnectorError> {
        let text = if self.change_link(&link.id, |l| l.muted_until = None).await {
            replies::unmuted()
        } else {
            replies::not_saved()
        };
        self.reply(place, &text).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::model::LinkThread;
    use crate::connectors::testing::link;
    use crate::hub::Channel;

    fn entry(channel_name: &str, id: &str, waiting_user: bool) -> ThreadEntry {
        ThreadEntry {
            channel: Channel::General,
            channel_name: channel_name.to_string(),
            thread_id: id.to_string(),
            title: format!("Titolo {id}"),
            waiting_user,
            updated_at: 0,
        }
    }

    #[test]
    fn mute_accepts_nothing_or_minutes_within_the_cap() {
        assert_eq!(parse_mute(""), Some(MuteRequest::UntilUnmute));
        assert_eq!(parse_mute(" 30 "), Some(MuteRequest::Minutes(30)));
        assert_eq!(parse_mute("1"), Some(MuteRequest::Minutes(1)));
        assert_eq!(parse_mute(&MUTE_MINUTES_MAX.to_string()), Some(MuteRequest::Minutes(MUTE_MINUTES_MAX)));
        for bad in ["0", "-5", "10081", "abc", "5m", "1.5", "99999999999999999999999"] {
            assert_eq!(parse_mute(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_mute_state_rounds_the_remaining_time_up() {
        assert_eq!(mute_state(None, 0), MuteState::Off);
        assert_eq!(mute_state(Some(1000), 1000), MuteState::Off, "scaduto");
        assert_eq!(mute_state(Some(500), 1000), MuteState::Off);
        assert_eq!(mute_state(Some(i64::MAX), 1000), MuteState::Forever);
        assert_eq!(mute_state(Some(1001), 1000), MuteState::Minutes(1));
        assert_eq!(mute_state(Some(1000 + 90_000), 1000), MuteState::Minutes(2));
    }

    #[test]
    fn waiting_threads_are_counted_per_channel_in_order_of_appearance() {
        let open = vec![
            entry("Sito", "a", true),
            entry("Generale", "b", true),
            entry("Sito", "c", true),
            entry("Generale", "d", false),
        ];
        assert_eq!(waiting_by_channel(&open), vec![("Sito".to_string(), 2), ("Generale".to_string(), 1)]);
        assert!(waiting_by_channel(&[entry("Sito", "x", false)]).is_empty());
    }

    #[test]
    fn the_current_thread_is_labelled_or_reported_gone() {
        let open = vec![entry("Generale", "thr-1", false)];
        let mut l = link("lnk-1-1", "u1", None);
        assert_eq!(current_label(&l, &open), "nessuno");
        l.current = Some(LinkThread { channel: Channel::General, thread_id: "thr-1".to_string() });
        assert_eq!(current_label(&l, &open), "Generale › Titolo thr-1");
        l.current = Some(LinkThread { channel: Channel::General, thread_id: "thr-9".to_string() });
        assert_eq!(current_label(&l, &open), "non più aperto");
    }
}
