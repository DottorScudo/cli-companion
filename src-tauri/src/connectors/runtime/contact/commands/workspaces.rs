//! `/ws [n|nome]`: il canale dove `/new` apre i thread.

use super::super::replies::{self, ChannelLine};
use super::super::ContactCtx;
use super::target_channel;
use crate::connectors::contracts::ConnectorError;
use crate::connectors::model::ConnectorLink;
use crate::hub::contact::ChannelEntry;

/// Il canale scelto da `arg`: il numero dell'ultimo elenco mostrato (se il canale
/// c'e' ancora) oppure il nome, senza badare alle maiuscole. Pura.
fn pick_channel(arg: &str, shown: &[ChannelEntry], current: &[ChannelEntry]) -> Option<ChannelEntry> {
    let arg = arg.trim();
    let numbered = arg
        .parse::<usize>()
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|index| shown.get(index))
        .filter(|entry| current.iter().any(|c| c.channel == entry.channel));
    if let Some(entry) = numbered {
        return Some(entry.clone());
    }
    let wanted = arg.to_lowercase();
    current.iter().find(|c| c.name.to_lowercase() == wanted).cloned()
}

impl ContactCtx<'_> {
    pub(super) async fn choose_workspace(&self, link: &ConnectorLink, arg: &str) -> Result<(), ConnectorError> {
        let chat = &link.external_chat;
        let channels = match self.hub.channels().await {
            Ok(channels) => channels,
            Err(e) => return self.reply(chat, &replies::hub_error(&e)).await,
        };
        if arg.trim().is_empty() {
            let chosen = target_channel(link);
            let lines: Vec<ChannelLine<'_>> =
                channels.iter().map(|c| ChannelLine { name: &c.name, chosen: c.channel == chosen }).collect();
            let text = replies::workspaces_list(&lines);
            self.session.with(&link.external_account, |s| s.remember_channels(channels));
            return self.reply(chat, &text).await;
        }
        let shown = self.session.with(&link.external_account, |s| s.listed_channels().to_vec());
        let Some(entry) = pick_channel(arg, &shown, &channels) else {
            return self.reply(chat, &replies::workspace_not_found()).await;
        };
        let chosen = entry.channel.clone();
        let text = if self.change_link(&link.id, move |l| l.channel = Some(chosen)).await {
            replies::workspace_chosen(&entry.name)
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

    fn entry(channel: Channel, name: &str) -> ChannelEntry {
        ChannelEntry { channel, name: name.to_string() }
    }

    fn channels() -> Vec<ChannelEntry> {
        vec![
            entry(Channel::General, "Generale"),
            entry(Channel::Workspace("ws-1".to_string()), "Sito"),
            entry(Channel::Workspace("ws-2".to_string()), "2024"),
        ]
    }

    #[test]
    fn a_number_picks_from_the_shown_listing() {
        let all = channels();
        assert_eq!(pick_channel("2", &all, &all), Some(all[1].clone()));
        assert_eq!(pick_channel("0", &all, &all), None);
        assert_eq!(pick_channel("9", &all, &all), None);
    }

    #[test]
    fn a_number_without_a_shown_listing_falls_back_to_the_name() {
        let all = channels();
        assert_eq!(pick_channel("2024", &[], &all), Some(all[2].clone()));
        assert_eq!(pick_channel("2", &[], &all), None);
    }

    #[test]
    fn a_name_matches_ignoring_case_and_spaces() {
        let all = channels();
        assert_eq!(pick_channel("  sITO ", &all, &all), Some(all[1].clone()));
        assert_eq!(pick_channel("generale", &[], &all), Some(all[0].clone()));
        assert_eq!(pick_channel("altro", &all, &all), None);
    }

    #[test]
    fn a_channel_that_disappeared_since_the_listing_is_not_picked() {
        let shown = channels();
        let now = vec![shown[0].clone()];
        assert_eq!(pick_channel("2", &shown, &now), None);
    }
}
