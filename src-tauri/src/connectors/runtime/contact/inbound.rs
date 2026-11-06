//! Un messaggio normale (non un comando) di un account collegato: in quale thread
//! va, quali allegati scaricare, la scrittura nell'HUB e la mappa che lega il
//! messaggio esterno a quello dell'HUB. Un file per responsabilita':
//! - `attachments`: quali allegati scaricare e con quale esito.

mod attachments;

use super::refs::MessageRef;
use super::replies;
use super::subchats::{SubchatKey, ThreadKey};
use super::{ContactCtx, Place};
use crate::connectors::contracts::{ConnectorError, InboundMessage};
use crate::connectors::model::{ConnectorLink, LinkThread};
use crate::hub::attachments::IncomingFile;
use crate::hub::send::UserMessage;
use crate::hub::{Channel, Message, MessageOrigin};

/// Dove va il messaggio.
struct Target {
    channel: Channel,
    thread_id: String,
    /// Il messaggio dello stesso thread a cui il messaggio risponde.
    reply_to: Option<String>,
    /// Rispondeva a un messaggio non piu' collegato: va scritto nel thread corrente e detto all'utente.
    stale_reply: bool,
}

impl ContactCtx<'_> {
    /// Il messaggio di un account collegato va al thread giusto, con gli allegati entro i limiti.
    pub(super) async fn forward(
        &self,
        link: &ConnectorLink,
        place: &Place,
        message: &InboundMessage,
    ) -> Result<(), ConnectorError> {
        let Some(target) = self.resolve_target(link, place, message).await? else { return Ok(()) };
        let (files, skipped) = self.collect_files(&message.files).await?;
        if !skipped.is_empty() {
            self.reply(place, &replies::attachments_skipped(&skipped)).await?;
        }
        if files.is_empty() && message.text.trim().is_empty() {
            return Ok(());
        }
        self.deliver(place, message, &target, files).await
    }

    /// Il thread di destinazione. `None` se non c'e' (l'utente ha gia' avuto la risposta del perche').
    /// Da una sotto-chat il suo thread; dalla chat principale la notifica a cui risponde, poi il corrente.
    async fn resolve_target(
        &self,
        link: &ConnectorLink,
        place: &Place,
        message: &InboundMessage,
    ) -> Result<Option<Target>, ConnectorError> {
        if let Some(subchat) = place.subchat.as_deref() {
            let key = SubchatKey { chat: place.chat.clone(), subchat: subchat.to_string() };
            return self.subchat_target(place, &key, message).await;
        }
        let mut stale_reply = false;
        if let Some(external) = message.reply_to_external.as_deref() {
            match self.replied_thread(link, place, external).await? {
                Replied::Thread(target) => return Ok(Some(target)),
                Replied::HubError => return Ok(None),
                Replied::Unlinked => stale_reply = true,
            }
        }
        self.current_target(link, place, stale_reply).await
    }

    /// Il thread della sotto-chat, se e' mappata e il thread esiste ancora; il thread
    /// corrente del collegamento non serve e non cambia. Altrimenti la spiegazione nella
    /// sotto-chat, e la voce di un thread sparito esce dalla mappa.
    async fn subchat_target(
        &self,
        place: &Place,
        key: &SubchatKey,
        message: &InboundMessage,
    ) -> Result<Option<Target>, ConnectorError> {
        let Some(thread) = self.thread_of_subchat(key) else {
            log::info!("connettori {}: messaggio ignorato, sotto-chat non legata a un thread", self.instance());
            self.reply(place, &replies::subchat_unmapped()).await?;
            return Ok(None);
        };
        match self.hub.thread_exists(&thread.channel, &thread.thread_id).await {
            Ok(true) => {}
            Ok(false) => {
                log::info!("connettori {}: thread della sotto-chat sparito, voce tolta", self.instance());
                self.forget_subchat(key);
                self.reply(place, &replies::subchat_thread_gone()).await?;
                return Ok(None);
            }
            Err(e) => {
                self.reply(place, &replies::not_delivered(&e)).await?;
                return Ok(None);
            }
        }
        let reply_to = message.reply_to_external.as_deref().and_then(|external| self.quoted_in(&thread, external));
        Ok(Some(Target { channel: thread.channel, thread_id: thread.thread_id, reply_to, stale_reply: false }))
    }

    /// Il messaggio dell'HUB dietro l'id esterno citato, se e' mappato e dello stesso thread.
    fn quoted_in(&self, thread: &ThreadKey, external: &str) -> Option<String> {
        let found = self.state.lock().external_to_hub.get(external).and_then(MessageRef::decode)?;
        (found.channel == thread.channel && found.thread_id == thread.thread_id).then_some(found.message_id)
    }

    /// La risposta a un messaggio che il runtime conosce: il thread di quel messaggio, che diventa il corrente.
    async fn replied_thread(
        &self,
        link: &ConnectorLink,
        place: &Place,
        external: &str,
    ) -> Result<Replied, ConnectorError> {
        let stored = self.state.lock().external_to_hub.get(external).map(str::to_string);
        let Some(found) = stored.as_deref().and_then(MessageRef::decode) else { return Ok(Replied::Unlinked) };
        match self.hub.thread_exists(&found.channel, &found.thread_id).await {
            Ok(true) => {}
            Ok(false) => return Ok(Replied::Unlinked),
            Err(e) => {
                self.reply(place, &replies::not_delivered(&e)).await?;
                return Ok(Replied::HubError);
            }
        }
        let already =
            link.current.as_ref().is_some_and(|c| c.channel == found.channel && c.thread_id == found.thread_id);
        if !already {
            let current = LinkThread { channel: found.channel.clone(), thread_id: found.thread_id.clone() };
            self.change_link(&link.id, move |l| l.current = Some(current)).await;
        }
        Ok(Replied::Thread(Target {
            channel: found.channel,
            thread_id: found.thread_id,
            reply_to: Some(found.message_id),
            stale_reply: false,
        }))
    }

    /// Il thread corrente del collegamento, se c'e' e esiste ancora.
    async fn current_target(
        &self,
        link: &ConnectorLink,
        place: &Place,
        stale_reply: bool,
    ) -> Result<Option<Target>, ConnectorError> {
        let Some(current) = link.current.clone() else {
            log::info!("connettori {}: messaggio ignorato, il collegamento non ha un thread corrente", self.instance());
            self.reply(place, &replies::no_current_thread()).await?;
            return Ok(None);
        };
        match self.hub.thread_exists(&current.channel, &current.thread_id).await {
            Ok(true) => {
                Ok(Some(Target { channel: current.channel, thread_id: current.thread_id, reply_to: None, stale_reply }))
            }
            Ok(false) => {
                log::info!("connettori {}: thread corrente sparito, collegamento azzerato", self.instance());
                self.change_link(&link.id, |l| l.current = None).await;
                self.reply(place, &replies::current_thread_gone()).await?;
                Ok(None)
            }
            Err(e) => {
                self.reply(place, &replies::not_delivered(&e)).await?;
                Ok(None)
            }
        }
    }

    /// Scrive nel thread. Se la citazione e' rifiutata (puo' essere di un altro thread) si riprova senza.
    /// Un errore dell'HUB non si riprova: l'utente lo legge e il messaggio si considera trattato.
    async fn deliver(
        &self,
        place: &Place,
        message: &InboundMessage,
        target: &Target,
        files: Vec<IncomingFile>,
    ) -> Result<(), ConnectorError> {
        let origin = MessageOrigin {
            connector: self.ctx.descriptor().id.to_string(),
            instance: self.instance().to_string(),
            external_id: message.external_id.clone(),
        };
        let build = |reply_to: Option<String>| UserMessage {
            channel: target.channel.clone(),
            thread_id: target.thread_id.clone(),
            body: message.text.clone(),
            reply_to,
            attachment_ids: Vec::new(),
            origin: Some(origin.clone()),
        };
        let mut sent = self.write_to_hub(build(target.reply_to.clone()), &files).await;
        if sent.is_err() && target.reply_to.is_some() {
            sent = self.write_to_hub(build(None), &files).await;
        }
        match sent {
            Ok(written) => {
                let stored = MessageRef {
                    channel: target.channel.clone(),
                    thread_id: target.thread_id.clone(),
                    message_id: written.id,
                };
                self.map_message(&message.external_id, &stored);
                if target.stale_reply {
                    self.reply(place, &replies::stale_reply()).await?;
                }
                Ok(())
            }
            Err(e) => {
                log::warn!("connettori {}: messaggio non consegnato all'HUB", self.instance());
                self.reply(place, &replies::not_delivered(&e)).await
            }
        }
    }

    async fn write_to_hub(&self, user: UserMessage, files: &[IncomingFile]) -> Result<Message, String> {
        if files.is_empty() {
            self.hub.send_user_message(user).await
        } else {
            self.hub.send_user_files(user, files.to_vec()).await
        }
    }
}

/// Come e' andata la ricerca del thread di un messaggio citato.
enum Replied {
    Thread(Target),
    /// Il messaggio citato non e' (piu') collegato a un thread esistente.
    Unlinked,
    /// L'HUB non risponde: l'utente l'ha gia' saputo.
    HubError,
}
