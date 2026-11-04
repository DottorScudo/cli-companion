//! Avvii integrati (disponibili in ogni workspace), derivati dal registro.
//!
//! Il menu offre: un avvio per ogni integrazione abilitata, una voce per ogni
//! preset che parte con l'agente del workspace (`builtin:preset:<id>`) e, per
//! ogni agente abilitato che dichiara i preset, le voci con l'agente
//! scelto (`builtin:preset:<id>@<agent>`).

use super::contracts::{AgentIntegration, PresetLauncher};
use super::enablement::{enabled_in, IntegrationsState};
use super::integrations_store;
use super::presets::{Preset, PRESETS};
use super::registry;
use super::resolve::{resolve_in, AgentUse};
use serde::Serialize;
use ts_rs::TS;

/// Avvio integrato derivato dal registro.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, TS)]
#[ts(export)]
pub struct BuiltinLaunch {
    /// `builtin:<agent_id>`, `builtin:preset:<preset_id>` oppure
    /// `builtin:preset:<preset_id>@<agent_id>`.
    pub id: String,
    pub name: String,
    pub command: String,
    /// Comando salvato nella sessione del tab per il ripristino: per un
    /// preset e' quello interattivo (riprende la conversazione invece di
    /// rieseguire la skill).
    pub restore_command: String,
    pub agent_id: String,
    pub agent_name: String,
    /// Id del preset; `None` per l'avvio interattivo di un agente.
    pub preset: Option<String>,
    /// La voce di un preset senza agente scelto: parte con quello del
    /// workspace (o il predefinito globale), qui gia' risolto in `agent_id`.
    pub follows_workspace_agent: bool,
}

fn agent_launch(integration: &dyn AgentIntegration) -> BuiltinLaunch {
    BuiltinLaunch {
        id: format!("builtin:{}", integration.id()),
        name: integration.display_name().to_string(),
        command: integration.interactive_command().to_string(),
        restore_command: integration.interactive_command().to_string(),
        agent_id: integration.id().to_string(),
        agent_name: integration.display_name().to_string(),
        preset: None,
        follows_workspace_agent: false,
    }
}

fn preset_launch(
    integration: &dyn AgentIntegration,
    launcher: &dyn PresetLauncher,
    preset: &Preset,
    follows_workspace_agent: bool,
) -> BuiltinLaunch {
    BuiltinLaunch {
        id: if follows_workspace_agent { preset.launch_id() } else { preset.agent_launch_id(integration.id()) },
        name: preset.name.to_string(),
        command: launcher.command(preset),
        restore_command: integration.interactive_command().to_string(),
        agent_id: integration.id().to_string(),
        agent_name: integration.display_name().to_string(),
        preset: Some(preset.id.to_string()),
        follows_workspace_agent,
    }
}

/// Avvii delle `integrations` (le abilitate). `resolved_agent` e' l'agente con
/// cui partono i preset senza agente scelto; senza (non risolvibile o senza
/// preset) quelle voci mancano. Le voci con agente scelto ci sono per ogni agente
/// che dichiara i preset.
pub(super) fn collect_builtin_launches(
    integrations: &[&dyn AgentIntegration],
    resolved_agent: Option<&str>,
    presets: &[Preset],
) -> Vec<BuiltinLaunch> {
    let mut out: Vec<BuiltinLaunch> = integrations.iter().map(|i| agent_launch(*i)).collect();
    let launchers: Vec<(&dyn AgentIntegration, &dyn PresetLauncher)> =
        integrations.iter().filter_map(|i| Some((*i, i.capabilities().presets?))).collect();
    // Prima le voci che seguono l'agente risolto, poi quelle con l'agente scelto
    // di ogni agente (anche del risolto: un id salvato deve restare valido).
    for follows_workspace_agent in [true, false] {
        for &(integration, launcher) in &launchers {
            if follows_workspace_agent && Some(integration.id()) != resolved_agent {
                continue;
            }
            out.extend(
                presets.iter().map(|preset| preset_launch(integration, launcher, preset, follows_workspace_agent)),
            );
        }
    }
    out
}

/// Come `builtin_launch_list_for`, su un registro e uno stato dati.
fn launches_in(
    registered: &[&dyn AgentIntegration],
    state: &IntegrationsState,
    workspace_agent: Option<&str>,
    presets: &[Preset],
) -> Vec<BuiltinLaunch> {
    let resolved = resolve_in(registered, state, None, workspace_agent, AgentUse::Preset).ok().map(|a| a.id());
    collect_builtin_launches(&enabled_in(registered, state), resolved, presets)
}

/// Avvii integrati per un workspace con `workspace_agent` (`None` = senza un
/// agente proprio): solo gli agenti abilitati, e i preset con l'agente che
/// `resolve_agent` sceglie per quel workspace. Usata anche per validare gli id.
pub fn builtin_launch_list_for(workspace_agent: Option<&str>) -> Vec<BuiltinLaunch> {
    launches_in(registry::all(), &integrations_store::current(), workspace_agent, PRESETS)
}

/// Avvii integrati con il predefinito globale come agente dei preset.
pub fn builtin_launch_list() -> Vec<BuiltinLaunch> {
    builtin_launch_list_for(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::testing::FakeIntegration;

    fn ids(launches: &[BuiltinLaunch]) -> Vec<&str> {
        launches.iter().map(|b| b.id.as_str()).collect()
    }

    fn state(enabled: &[&str], default_agent: &str) -> IntegrationsState {
        IntegrationsState {
            enabled: enabled.iter().map(|s| s.to_string()).collect(),
            default_agent: default_agent.into(),
        }
    }

    #[test]
    fn builtin_launches_from_registry() {
        let l = builtin_launch_list();
        assert_eq!(l[0].id, "builtin:claude-code");
        assert_eq!(l[0].name, "Claude Code");
        assert_eq!(l[0].command, l[0].restore_command);
        assert_eq!((l[0].preset.as_deref(), l[0].follows_workspace_agent), (None, false));
        assert!(!l.iter().any(|b| b.id == "builtin:orchestrator"));
        assert_eq!(
            ids(&l),
            vec![
                "builtin:claude-code",
                "builtin:preset:task-intake",
                "builtin:preset:skill-wizard",
                "builtin:preset:project-setup",
                "builtin:preset:task-intake@claude-code",
                "builtin:preset:skill-wizard@claude-code",
                "builtin:preset:project-setup@claude-code"
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
        assert_eq!((p.agent_id.as_str(), p.agent_name.as_str()), ("claude-code", "Claude Code"));
        assert_eq!((p.preset.as_deref(), p.follows_workspace_agent), (Some("task-intake"), true));
        assert_eq!(p.restore_command, l[0].command);
        assert_eq!(p.command, "claude --dangerously-skip-permissions \"/cli-companion-notify:task-intake\"");
    }

    fn preset(id: &'static str, name: &'static str) -> Preset {
        Preset { id, name, skill: "" }
    }

    #[test]
    fn presets_follow_the_resolved_agent_and_every_agent_has_explicit_entries() {
        let a = FakeIntegration::presets_only("a", "a-cli", "a-run");
        let b = FakeIntegration::presets_only("b", "b-cli", "b-run");
        let c = FakeIntegration::bare("c", "c-cli");
        let presets = [preset("one", "One"), preset("two", "Two")];
        let l = collect_builtin_launches(&[&a, &b, &c], Some("b"), &presets);
        assert_eq!(
            ids(&l),
            vec![
                "builtin:a",
                "builtin:b",
                "builtin:c",
                "builtin:preset:one",
                "builtin:preset:two",
                "builtin:preset:one@a",
                "builtin:preset:two@a",
                "builtin:preset:one@b",
                "builtin:preset:two@b"
            ]
        );
        assert_eq!((l[3].name.as_str(), l[3].command.as_str()), ("One", "b-run one"));
        assert_eq!((l[3].restore_command.as_str(), l[3].agent_id.as_str()), ("b-cli", "b"));
        assert!(l[3].follows_workspace_agent);
        assert_eq!(
            (l[5].command.as_str(), l[5].restore_command.as_str(), l[5].agent_id.as_str()),
            ("a-run one", "a-cli", "a")
        );
        assert!(!l[5].follows_workspace_agent);
        assert_eq!(l[5].preset.as_deref(), Some("one"));
    }

    #[test]
    fn without_a_resolved_agent_only_the_explicit_entries_remain() {
        let a = FakeIntegration::presets_only("a", "a-cli", "a-run");
        let c = FakeIntegration::bare("c", "c-cli");
        let presets = [preset("one", "One")];
        for resolved in [None, Some("c"), Some("nope")] {
            let l = collect_builtin_launches(&[&a, &c], resolved, &presets);
            assert_eq!(ids(&l), vec!["builtin:a", "builtin:c", "builtin:preset:one@a"], "{resolved:?}");
        }
    }

    #[test]
    fn agents_without_presets_have_only_the_interactive_launch() {
        let c = FakeIntegration::bare("c", "c-cli");
        let l = collect_builtin_launches(&[&c], Some("c"), &[preset("one", "One")]);
        assert_eq!(ids(&l), vec!["builtin:c"]);
        assert_eq!(l[0].restore_command, "c-cli");
    }

    #[test]
    fn the_preset_agent_is_the_workspace_one_then_the_global_default() {
        let a = FakeIntegration::presets_only("a", "a-cli", "a-run");
        let b = FakeIntegration::presets_only("b", "b-cli", "b-run");
        let registered: [&dyn AgentIntegration; 2] = [&a, &b];
        let both = state(&["a", "b"], "a");
        let presets = [preset("one", "One")];
        let preset_agent = |workspace_agent| {
            let l = launches_in(&registered, &both, workspace_agent, &presets);
            l.into_iter().find(|b| b.follows_workspace_agent).map(|b| b.agent_id)
        };
        assert_eq!(preset_agent(None).as_deref(), Some("a"));
        assert_eq!(preset_agent(Some("b")).as_deref(), Some("b"));
        let b_default = state(&["a", "b"], "b");
        let l = launches_in(&registered, &b_default, None, &presets);
        assert_eq!(l.iter().find(|x| x.follows_workspace_agent).map(|x| x.agent_id.as_str()), Some("b"));
    }

    #[test]
    fn disabled_agents_have_no_launches_and_cannot_run_the_presets() {
        let a = FakeIntegration::presets_only("a", "a-cli", "a-run");
        let b = FakeIntegration::presets_only("b", "b-cli", "b-run");
        let c = FakeIntegration::bare("c", "c-cli");
        let registered: [&dyn AgentIntegration; 3] = [&a, &b, &c];
        let presets = [preset("one", "One")];
        let only_a = state(&["a"], "a");
        // L'agente del workspace e' disabilitato: nessun ripiego su un altro agente.
        let l = launches_in(&registered, &only_a, Some("b"), &presets);
        assert_eq!(ids(&l), vec!["builtin:a", "builtin:preset:one@a"]);
        // Sconosciuto, o senza il contratto dei preset: stessa cosa.
        let a_and_c = state(&["a", "c"], "a");
        for workspace_agent in ["gone", "c"] {
            let l = launches_in(&registered, &a_and_c, Some(workspace_agent), &presets);
            assert_eq!(ids(&l), vec!["builtin:a", "builtin:c", "builtin:preset:one@a"], "{workspace_agent}");
        }
    }
}
