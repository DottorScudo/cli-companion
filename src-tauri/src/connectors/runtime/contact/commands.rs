//! L'esecuzione dei comandi di un account collegato (`/threads`, `/use`...):
//! ogni comando risponde nella chat. Un file per gruppo:
//! - `threads`: `/threads`, `/use`, `/new`;
//! - `workspaces`: `/ws`;
//! - `status`: `/status`, `/mute`, `/unmute`.

mod status;
mod threads;
mod workspaces;

use super::replies;
use super::{ContactCtx, Place};
use crate::connectors::contracts::{ConnectorError, ContactCommand};
use crate::connectors::model::{ConnectorLink, LinkThread};
use crate::hub::Channel;

/// Il canale dove `/new` apre i thread: quello scelto con `/ws`, altrimenti
/// quello del thread corrente, altrimenti il Generale. Pura.
fn target_channel(link: &ConnectorLink) -> Channel {
    link.channel
        .clone()
        .or_else(|| link.current.as_ref().map(|current| current.channel.clone()))
        .unwrap_or(Channel::General)
}

impl ContactCtx<'_> {
    /// Esegue `command` per `link`, rispondendo in `place` (la chat o la sotto-chat del
    /// comando: dentro una sotto-chat i comandi valgono come nella chat principale). Un
    /// errore che ferma il supervisore risale.
    pub(super) async fn run_command(
        &self,
        link: &ConnectorLink,
        place: &Place,
        command: ContactCommand,
    ) -> Result<(), ConnectorError> {
        match command {
            ContactCommand::Help => self.reply(place, &self.help_for(&link.external_chat)).await,
            ContactCommand::Unknown => self.reply(place, &self.unknown_for(&link.external_chat)).await,
            ContactCommand::Threads => self.list_threads(link, place).await,
            ContactCommand::Use(arg) => self.use_thread(link, place, &arg).await,
            ContactCommand::Workspaces(arg) => self.choose_workspace(link, place, &arg).await,
            ContactCommand::New(title) => self.new_thread(link, place, &title).await,
            ContactCommand::Status => self.show_status(link, place).await,
            ContactCommand::Mute(arg) => self.mute(link, place, &arg).await,
            ContactCommand::Unmute => self.unmute(link, place).await,
            ContactCommand::Unlink => self.unlink(link, place).await,
        }
    }

    /// La risposta a un comando sconosciuto, con l'aiuto adatto alla chat.
    fn unknown_for(&self, chat: &str) -> String {
        if self.has_subchats(chat) {
            replies::unknown_subchat_command()
        } else {
            replies::unknown_command()
        }
    }

    /// `/unlink`: prima la risposta (dopo il collegamento non ci sarebbe piu' a chi), poi via dal file.
    async fn unlink(&self, link: &ConnectorLink, place: &Place) -> Result<(), ConnectorError> {
        let replied = self.reply(place, &replies::unlinked()).await;
        self.remove_link(link).await;
        replied
    }

    /// Il thread `channel`/`thread_id` diventa il corrente del collegamento, e il suo canale quello dei thread nuovi.
    async fn set_current(&self, link: &ConnectorLink, channel: &Channel, thread_id: &str) -> bool {
        let current = LinkThread { channel: channel.clone(), thread_id: thread_id.to_string() };
        let chosen = channel.clone();
        self.change_link(&link.id, move |l| {
            l.current = Some(current);
            l.channel = Some(chosen);
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::testing::link;

    #[test]
    fn new_threads_go_to_the_chosen_channel_then_the_current_one_then_general() {
        let ws = |id: &str| Channel::Workspace(id.to_string());
        let mut l = link("lnk-1-1", "u1", None);
        assert_eq!(target_channel(&l), Channel::General);
        l.current = Some(LinkThread { channel: ws("ws-1"), thread_id: "thr-1".to_string() });
        assert_eq!(target_channel(&l), ws("ws-1"));
        l.channel = Some(ws("ws-2"));
        assert_eq!(target_channel(&l), ws("ws-2"));
    }
}
