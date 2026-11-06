//! Un Bot non puo' portare il nome di un segretario dell'HUB (`agent_bots::ensure_free`),
//! ma i passi di migrazione di `bots.json` non conoscono i segretari: sono funzioni
//! pure del solo `bots.json` (`store::Migration`), che si carica prima di
//! `hub_settings.json`. Per questo, caricati entrambi, un Bot che porta il nome di un
//! segretario prende il primo nome libero ("Researcher 2"), cosi' l'editor non ne
//! rifiuta il salvataggio. Vale per i Bot di ogni migrazione, presente e futura.

use super::agent::AgentBot;
use super::agent_bots::free_name;
use super::commands::secretary_names;
use super::{update, BotSettingsStore};
use tauri::{AppHandle, Manager};

/// All'avvio, dopo il caricamento di Bot e segretari: rinomina e salva i Bot che
/// portano il nome di un segretario. Se nessuno lo porta non scrive nulla.
pub fn rename_bots_named_like_secretaries(app: &AppHandle) {
    let taken = secretary_names(app);
    let clashing = app.state::<BotSettingsStore>().read(|s| s.agents.iter().any(|b| is_taken(b, &taken)));
    if !clashing {
        return;
    }
    let saved = update(app, |s| {
        free_secretary_names(&mut s.agents, &taken);
        Ok(())
    });
    if let Err(e) = saved {
        log::error!("bots: i Bot con il nome di un segretario non sono stati rinominati: {e}");
    }
}

/// Da' il primo nome libero (fra i Bot e fra `taken`, i nomi dei segretari in
/// minuscolo) a ogni Bot che porta un nome di `taken`. Gli altri non cambiano.
fn free_secretary_names(list: &mut [AgentBot], taken: &[String]) {
    for at in 0..list.len() {
        if is_taken(&list[at], taken) {
            let old = list[at].name.clone();
            let name = free_name(list, &old, taken);
            log::info!("bots: il Bot {} passa da “{old}” a “{name}”: il nome è di un segretario", list[at].id);
            list[at].name = name;
        }
    }
}

fn is_taken(bot: &AgentBot, taken: &[String]) -> bool {
    taken.contains(&bot.name.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bots::presets::default_bots;

    #[test]
    fn a_bot_named_like_a_secretary_gets_the_first_free_name() {
        let mut list = default_bots();
        list[1].name = "Reviewer 2".into();
        free_secretary_names(&mut list, &["reviewer".to_string(), "planner".to_string()]);
        let reviewer = list.iter().find(|b| b.id == "reviewer").unwrap();
        let planner = list.iter().find(|b| b.id == "planner").unwrap();
        assert_eq!((reviewer.name.as_str(), planner.name.as_str()), ("Reviewer 3", "Planner 2"));
        assert_eq!(list[1].name, "Reviewer 2", "a free name is kept");
    }

    #[test]
    fn without_clashes_nothing_changes() {
        let mut list = default_bots();
        let before = list.clone();
        free_secretary_names(&mut list, &["mario".to_string(), "luca".to_string()]);
        assert_eq!(list, before);
        free_secretary_names(&mut list, &[]);
        assert_eq!(list, before);
    }
}
