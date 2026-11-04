//! Avvii integrati (disponibili in ogni workspace), derivati dal registro.

use super::contracts::{AgentIntegration, PresetLauncher};
use super::presets::{Preset, PRESETS};
use super::registry::{self, default_worker_integration};
use serde::Serialize;
use ts_rs::TS;

/// Avvio integrato derivato dal registro.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, TS)]
#[ts(export)]
pub struct BuiltinLaunch {
    /// `builtin:<agent_id>` oppure `builtin:preset:<preset_id>`.
    pub id: String,
    pub name: String,
    pub command: String,
    /// Comando salvato nella sessione del tab per il ripristino: per un
    /// preset e' quello interattivo (riprende la conversazione invece di
    /// rieseguire la skill).
    pub restore_command: String,
    pub agent_id: String,
}

pub(super) fn collect_builtin_launches(
    integrations: &[&dyn AgentIntegration],
    default_agent: &str,
    presets: &[Preset],
) -> Vec<BuiltinLaunch> {
    let mut out: Vec<BuiltinLaunch> = integrations
        .iter()
        .map(|i| BuiltinLaunch {
            id: format!("builtin:{}", i.id()),
            name: i.display_name().to_string(),
            command: i.interactive_command().to_string(),
            restore_command: i.interactive_command().to_string(),
            agent_id: i.id().to_string(),
        })
        .collect();
    // Un avvio per preset: l'agente di default se li dichiara, altrimenti il primo che puo'.
    let launchers: Vec<(&dyn AgentIntegration, &dyn PresetLauncher)> =
        integrations.iter().filter_map(|i| Some((*i, i.capabilities().presets?))).collect();
    let chosen = launchers.iter().find(|(i, _)| i.id() == default_agent).or_else(|| launchers.first());
    let Some((integration, launcher)) = chosen else { return out };
    for preset in presets {
        out.push(BuiltinLaunch {
            id: preset.launch_id(),
            name: preset.name.to_string(),
            command: launcher.command(preset),
            restore_command: integration.interactive_command().to_string(),
            agent_id: integration.id().to_string(),
        });
    }
    out
}

/// Avvii integrati di tutti gli agenti (usata anche per validare gli id).
pub fn builtin_launch_list() -> Vec<BuiltinLaunch> {
    collect_builtin_launches(registry::all(), default_worker_integration().id(), PRESETS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::testing::FakeIntegration;

    #[test]
    fn builtin_launches_from_registry() {
        let l = builtin_launch_list();
        assert_eq!(l[0].id, "builtin:claude-code");
        assert_eq!(l[0].name, "Claude Code");
        assert_eq!(l[0].command, l[0].restore_command);
        assert!(!l.iter().any(|b| b.id == "builtin:orchestrator"));
        let ids: Vec<_> = l.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "builtin:claude-code",
                "builtin:preset:task-intake",
                "builtin:preset:skill-wizard",
                "builtin:preset:project-setup"
            ]
        );
        let s = l.iter().find(|b| b.id == "builtin:preset:project-setup").unwrap();
        assert_eq!(s.name, "Project Setup");
        assert_eq!(s.command, "claude --dangerously-skip-permissions \"/cli-companion-notify:project-setup\"");
        let w = l.iter().find(|b| b.id == "builtin:preset:skill-wizard").unwrap();
        assert_eq!(w.name, "Skill Wizard");
        assert_eq!(w.command, "claude --dangerously-skip-permissions \"/cli-companion-notify:skill-wizard\"");
        let p = l.iter().find(|b| b.id == "builtin:preset:task-intake").unwrap();
        assert_eq!(p.name, "Task Intake");
        assert_eq!(p.agent_id, "claude-code");
        assert_eq!(p.restore_command, l[0].command);
        assert_eq!(p.command, "claude --dangerously-skip-permissions \"/cli-companion-notify:task-intake\"");
    }

    fn preset(id: &'static str, name: &'static str) -> Preset {
        Preset { id, name, skill: "" }
    }

    #[test]
    fn preset_launches_prefer_default_agent_and_may_be_absent() {
        let a = FakeIntegration::presets_only("a", "a-cli", "a-run");
        let b = FakeIntegration::presets_only("b", "b-cli", "b-run");
        let c = FakeIntegration::bare("c", "c-cli");
        let presets = [preset("one", "One"), preset("two", "Two")];
        let l = collect_builtin_launches(&[&a, &b, &c], "b", &presets);
        let ids: Vec<_> = l.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, vec!["builtin:a", "builtin:b", "builtin:c", "builtin:preset:one", "builtin:preset:two"]);
        assert_eq!((l[3].name.as_str(), l[3].command.as_str()), ("One", "b-run one"));
        assert_eq!(l[3].restore_command, "b-cli");
        assert_eq!(l[4].agent_id, "b");
        // Default senza preset: il primo agente che li dichiara.
        let l = collect_builtin_launches(&[&c, &a], "c", &presets);
        assert_eq!(l.last().unwrap().agent_id, "a");
        // Nessun agente con preset: nessuna voce preset, solo gli avvii interattivi.
        let l = collect_builtin_launches(&[&c], "c", &presets);
        assert_eq!(l.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), vec!["builtin:c"]);
        assert_eq!(l[0].restore_command, "c-cli");
    }
}
