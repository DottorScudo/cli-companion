//! `/threads`, `/use <n>` e `/new <titolo>`.

use super::super::replies::{self, ThreadLine};
use super::super::session::ListedThread;
use super::super::ContactCtx;
use super::target_channel;
use crate::connectors::contracts::ConnectorError;
use crate::connectors::model::ConnectorLink;
use crate::hub::contact::ThreadEntry;

/// Thread mostrati al massimo da `/threads`.
const THREADS_LISTED: usize = 10;

/// I thread da mostrare: prima quelli che aspettano l'utente, poi i piu' recenti; al piu' `THREADS_LISTED`. Pura.
fn listing(mut entries: Vec<ThreadEntry>) -> Vec<ThreadEntry> {
    entries.sort_by(|a, b| b.waiting_user.cmp(&a.waiting_user).then(b.updated_at.cmp(&a.updated_at)));
    entries.truncate(THREADS_LISTED);
    entries
}

/// Il numero scritto dopo `/use` (da 1). Puro.
fn parse_number(arg: &str) -> Option<usize> {
    arg.trim().parse::<usize>().ok().filter(|n| *n >= 1)
}

impl ContactCtx<'_> {
    pub(super) async fn list_threads(&self, link: &ConnectorLink) -> Result<(), ConnectorError> {
        let chat = &link.external_chat;
        let entries = match self.hub.open_threads().await {
            Ok(entries) => listing(entries),
            Err(e) => return self.reply(chat, &replies::hub_error(&e)).await,
        };
        let listed: Vec<ListedThread> = entries
            .iter()
            .map(|e| ListedThread {
                channel: e.channel.clone(),
                thread_id: e.thread_id.clone(),
                label: replies::thread_label(&e.channel_name, &e.title),
            })
            .collect();
        let text = if listed.is_empty() {
            replies::no_threads()
        } else {
            let lines: Vec<ThreadLine<'_>> = listed
                .iter()
                .zip(&entries)
                .map(|(t, e)| ThreadLine { label: &t.label, waiting: e.waiting_user })
                .collect();
            replies::threads_list(&lines)
        };
        self.session.with(&link.external_account, |s| s.remember_threads(listed));
        self.reply(chat, &text).await
    }

    pub(super) async fn use_thread(&self, link: &ConnectorLink, arg: &str) -> Result<(), ConnectorError> {
        let chat = &link.external_chat;
        let (picked, listed) = self.session.with(&link.external_account, |s| {
            (parse_number(arg).and_then(|n| s.thread_at(n).cloned()), s.listed_threads())
        });
        let Some(picked) = picked else { return self.reply(chat, &replies::use_usage(listed)).await };
        match self.hub.thread_exists(&picked.channel, &picked.thread_id).await {
            Ok(true) => {}
            Ok(false) => return self.reply(chat, &replies::thread_gone()).await,
            Err(e) => return self.reply(chat, &replies::hub_error(&e)).await,
        }
        let text = if self.set_current(link, &picked.channel, &picked.thread_id).await {
            replies::now_using(&picked.label)
        } else {
            replies::not_saved()
        };
        self.reply(chat, &text).await
    }

    pub(super) async fn new_thread(&self, link: &ConnectorLink, title: &str) -> Result<(), ConnectorError> {
        let chat = &link.external_chat;
        let title = title.trim();
        if title.is_empty() {
            return self.reply(chat, &replies::new_usage()).await;
        }
        let channel = target_channel(link);
        let thread_id = match self.hub.create_thread(&channel, title).await {
            Ok(id) => id,
            Err(e) => return self.reply(chat, &replies::new_failed(&e)).await,
        };
        let text = if self.set_current(link, &channel, &thread_id).await {
            replies::thread_created(title)
        } else {
            replies::not_saved()
        };
        self.reply(chat, &text).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::Channel;

    fn entry(id: &str, waiting_user: bool, updated_at: i64) -> ThreadEntry {
        ThreadEntry {
            channel: Channel::General,
            channel_name: "Generale".to_string(),
            thread_id: id.to_string(),
            title: id.to_string(),
            waiting_user,
            updated_at,
        }
    }

    fn ids(entries: &[ThreadEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.thread_id.as_str()).collect()
    }

    #[test]
    fn waiting_threads_come_first_then_the_most_recent() {
        let sorted = listing(vec![
            entry("old", false, 1),
            entry("new", false, 9),
            entry("wait-old", true, 2),
            entry("wait-new", true, 8),
        ]);
        assert_eq!(ids(&sorted), ["wait-new", "wait-old", "new", "old"]);
    }

    #[test]
    fn the_listing_is_capped() {
        let many: Vec<ThreadEntry> =
            (0..THREADS_LISTED + 5).map(|i| entry(&format!("t{i}"), false, i as i64)).collect();
        let shown = listing(many);
        assert_eq!(shown.len(), THREADS_LISTED);
        assert_eq!(shown[0].thread_id, format!("t{}", THREADS_LISTED + 4));
    }

    #[test]
    fn only_positive_numbers_are_accepted() {
        assert_eq!(parse_number(" 3 "), Some(3));
        for bad in ["", "0", "-1", "a", "1.5", "2 3", "99999999999999999999999"] {
            assert_eq!(parse_number(bad), None, "{bad:?}");
        }
    }
}
