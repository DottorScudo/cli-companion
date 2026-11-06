//! Dalle vecchie voci di lancio dei preset ai Bot.
//!
//! - `ids`: la corrispondenza congelata fra le vecchie voci e i Bot, usata dalle
//!   migrazioni di `workspaces.json` e `launch_settings.json` e per gli id letti
//!   dopo il caricamento;
//! - `reconcile`: il passo puro che crea i Bot derivati che le migrazioni non
//!   possono creare (stanno in `bots.json`);
//! - qui: il passo applicato all'avvio, dopo il caricamento degli store.

mod ids;
mod reconcile;

pub use ids::{current_launch_id, migrate_launch_value};

use crate::agents::registry;
use crate::bots::commands::secretary_names;
use crate::bots::BotSettingsStore;
use crate::launch_settings::LaunchSettingsStore;
use crate::store::data_dir;
use crate::workspaces::WorkspaceStore;
use reconcile::{reconcile, BotsContext, Reconciliation};
use std::path::Path;
use tauri::Manager;

/// Applica la riconciliazione: prima i Bot (cosi' le voci riscritte puntano a
/// un Bot che c'e'), poi i workspace e l'avvio globale. Un salvataggio fallito
/// si registra: alla prossima accensione il passo riprova.
pub fn reconcile_at_startup(app: &tauri::App) {
    let dir = match data_dir(app.handle()) {
        Ok(dir) => dir,
        Err(e) => {
            log::error!("cartella dati non disponibile, avvii predefiniti non riconciliati: {e}");
            return;
        }
    };
    let bots = app.state::<BotSettingsStore>().read(|s| s.agents.clone());
    let taken = secretary_names(app.handle());
    let context = BotsContext {
        bots: &bots,
        taken: &taken,
        agent_name: |id: &str| registry::find(id).ok().map(|a| a.display_name().to_string()),
    };
    let plan = reconcile(&app.state::<WorkspaceStore>().all(), &app.state::<LaunchSettingsStore>().get(), &context);
    if !plan.is_empty() {
        apply(app, &dir, plan);
    }
}

fn apply(app: &tauri::App, dir: &Path, plan: Reconciliation) {
    let Reconciliation { workspaces, global_default, new_bots } = plan;
    if !new_bots.is_empty() {
        let saved = app.state::<BotSettingsStore>().update(dir, |s| {
            let fresh: Vec<_> = new_bots.into_iter().filter(|b| !s.agents.iter().any(|a| a.id == b.id)).collect();
            s.agents.extend(fresh);
            Ok(())
        });
        if let Err(e) = saved {
            log::error!("Bot derivati dalle vecchie voci dei preset non salvati: {e}");
            return;
        }
    }
    if !workspaces.is_empty() {
        let saved = app.state::<WorkspaceStore>().update(dir, |list| {
            for (id, launch) in workspaces {
                if let Some(ws) = list.iter_mut().find(|w| w.id == id) {
                    ws.default_launch = Some(launch);
                }
            }
            Ok(())
        });
        if let Err(e) = saved {
            log::error!("avvii predefiniti dei workspace non riconciliati: {e}");
        }
    }
    if let Some(launch) = global_default {
        let saved = app.state::<LaunchSettingsStore>().update(dir, |s| {
            s.default_launch = Some(launch);
            Ok(())
        });
        if let Err(e) = saved {
            log::error!("avvio predefinito globale non riconciliato: {e}");
        }
    }
}
