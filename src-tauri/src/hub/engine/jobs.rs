//! I lavori della coda di un thread, eseguiti da un task async alla volta: il
//! turno di un segretario (`reply`) e il riassunto alla chiusura di una
//! sessione (`summarize`). Le impostazioni si rileggono a ogni lavoro: un
//! cambio di provider, modello, lingua, passi o segretari vale dal turno
//! successivo senza chiudere le sessioni aperte; un segretario tolto, svuotato
//! o escluso dal canale chiude la sua sessione al turno successivo.

use super::chat::{ChatMessage, ModelError};
use super::context::{self, Trigger};
use super::host::{AppTurnHost, ThreadIo};
use super::planning::proposal_target;
use super::relay::relay_candidates;
use super::runtime::SecretaryEngine;
use super::sessions::{usable, Job, ThreadKey};
use super::texts;
use super::turn::{compact, run_turn, updated_summary, TurnEnd, TurnSpec};
use super::workspace_names::WorkspaceNames;
use crate::hub::model::Author;
use crate::hub::settings::HubSettingsStore;
use crate::hub::text::clean_text;
use crate::hub::text::parts::{self, Remainder};
use crate::workspaces::WorkspaceStore;
use tauri::{AppHandle, Manager};

/// Svuota la coda del thread, un lavoro alla volta, e si ferma quando e'
/// vuota o l'HUB e' stato spento (epoca cambiata).
pub async fn run_queue(app: AppHandle, key: ThreadKey, epoch: u64) {
    let engine = app.state::<SecretaryEngine>();
    while let Some(job) = engine.next_job(&key, epoch) {
        engine.emit_status(&app);
        match job {
            Job::Reply { secretary, trigger, cause } => {
                let trigger = Trigger { id: &trigger, cause, proposal: None };
                reply(&app, &key, &secretary, trigger, epoch).await
            }
            Job::Summarize { secretary, conversation } => summarize(&app, &key, &secretary, &conversation, epoch).await,
        }
        engine.job_done(&key, epoch);
        engine.emit_status(&app);
    }
}

/// Un turno del segretario `name` per il messaggio `trigger`. La proposta che
/// il turno puo' decidere e la domanda a cui puo' rispondere si ricavano qui,
/// dal thread letto all'inizio del turno.
async fn reply(app: &AppHandle, key: &ThreadKey, name: &str, mut trigger: Trigger<'_>, epoch: u64) {
    let engine = app.state::<SecretaryEngine>();
    let settings = app.state::<HubSettingsStore>().get();
    let io = ThreadIo::new(app, key, epoch);
    let Some(secretary) = usable(&settings.secretaries, name, &key.channel) else {
        engine.close(key, name);
        io.notice(texts::secretary_gone(name)).await;
        return;
    };
    let thread = match io.load_thread().await {
        Ok(thread) => thread,
        Err(e) => {
            log::warn!("hub: thread {} non leggibile, sessione di {name} chiusa: {e}", key.thread_id);
            engine.close(key, name);
            return;
        }
    };
    let Some(mut turn) = engine.begin_turn(key, name, epoch) else {
        return;
    };
    let names = current_workspace_names(app);
    let proposal = proposal_target(&thread, trigger.id, trigger.cause);
    let question = relay_candidates(&thread, trigger.cause);
    trigger.proposal = proposal.as_ref().map(|p| p.proposal_id.as_str());
    let Some(opening) = context::turn_message(&thread, &trigger, turn.last_seen.as_deref(), name, &names) else {
        log::warn!("hub: messaggio {} non piu' nel thread, turno di {name} saltato", trigger.id);
        engine.end_turn(key, name, turn, epoch);
        return;
    };
    let model = match io.model(&settings.provider).await {
        Ok(model) => model,
        Err(e) => {
            io.notice(texts::model_error(name, e)).await;
            engine.end_turn(key, name, turn, epoch);
            return;
        }
    };
    turn.conversation.push(ChatMessage::User(opening));
    let max_steps = settings.session.max_steps;
    let spec = TurnSpec {
        secretary,
        system: context::system_prompt(
            secretary,
            settings.language,
            &key.channel,
            &names,
            &crate::bots::agent_bots(app),
        ),
        max_steps,
    };
    let host = AppTurnHost {
        io: &io,
        secretary,
        trigger: trigger.id,
        cause: trigger.cause,
        proposal: proposal.clone(),
        question,
        names: &names,
    };
    match run_turn(&model, &host, &spec, &mut turn.conversation).await {
        TurnEnd::Reply(text) => post_reply(&io, name, &text).await,
        TurnEnd::StepLimit => io.notice(texts::step_limit(name, max_steps)).await,
        TurnEnd::Failed(e) => io.notice(texts::model_error(name, e)).await,
    }
    turn.last_seen = Some(trigger.id.to_string());
    compact(&mut turn.conversation);
    engine.end_turn(key, name, turn, epoch);
}

/// La risposta finale nel thread: una risposta piu' lunga di un messaggio
/// diventa piu' messaggi consecutivi (`reply_parts`).
async fn post_reply(io: &ThreadIo, name: &str, text: &str) {
    let parts = reply_parts(text);
    if parts.is_empty() {
        io.notice(texts::model_error(name, ModelError::Empty)).await;
        return;
    }
    if let Err(e) = io.append_parts(Author::Bot { name: name.to_string() }, parts).await {
        log::error!("hub: risposta di {name} non scritta: {e}");
    }
}

/// La risposta ripulita e divisa nei messaggi che la portano: vuota se non
/// resta testo; oltre `parts::MAX_PARTS` l'ultima parte dice quanto manca.
fn reply_parts(text: &str) -> Vec<String> {
    parts::split(&clean_text(text, usize::MAX), Remainder::Lost)
}

/// Il riassunto del thread dalla conversazione della sessione chiusa; se non
/// si riesce a scriverlo resta il precedente (l'errore e' nel log).
async fn summarize(app: &AppHandle, key: &ThreadKey, name: &str, conversation: &[ChatMessage], epoch: u64) {
    let settings = app.state::<HubSettingsStore>().get();
    let io = ThreadIo::new(app, key, epoch);
    let thread = match io.load_thread().await {
        Ok(thread) => thread,
        Err(e) => {
            log::warn!("hub: riassunto di {name} non scritto, thread {} non leggibile: {e}", key.thread_id);
            return;
        }
    };
    let model = match io.model(&settings.provider).await {
        Ok(model) => model,
        Err(e) => {
            log::warn!("hub: riassunto di {name} non scritto ({e:?}), resta il precedente");
            return;
        }
    };
    let Some(summary) = updated_summary(&model, thread.summary.as_deref(), conversation).await else {
        return;
    };
    if let Err(e) = io.save_summary(summary).await {
        log::error!("hub: riassunto di {name} non salvato: {e}");
    }
}

/// I workspace di adesso (nome compreso: un rinominato si vede subito), letti
/// dallo store in memoria all'inizio del turno.
fn current_workspace_names(app: &AppHandle) -> WorkspaceNames {
    WorkspaceNames::new(app.state::<WorkspaceStore>().all().into_iter().map(|w| (w.id, w.name)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::text::BODY_MAX;

    #[test]
    fn a_long_reply_becomes_consecutive_messages_that_rebuild_it() {
        let paragraph = "Il task è in coda dietro la migrazione del database. ".repeat(10);
        let text = vec![paragraph.trim_end(); 40].join("\n\n");
        assert!(text.chars().count() >= 20_000);
        let parts = reply_parts(&format!("  {text}\u{7}\n"));
        assert_eq!(parts.len(), 3);
        let rebuilt: String = parts
            .iter()
            .enumerate()
            .map(|(i, part)| {
                assert!(part.chars().count() <= BODY_MAX);
                part.strip_prefix(&format!("(parte {}/3)\n", i + 1)).unwrap()
            })
            .collect();
        assert_eq!(rebuilt, text);
    }

    #[test]
    fn a_short_reply_is_one_message_and_an_empty_one_none() {
        assert_eq!(reply_parts(" Fatto.\u{1b} "), ["Fatto."]);
        assert!(reply_parts(" \u{7}\n ").is_empty());
    }
}
