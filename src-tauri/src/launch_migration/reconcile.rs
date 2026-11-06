//! Riconciliazione pura degli avvii predefiniti con i Bot, dopo il caricamento
//! degli store. Le migrazioni dei file riscrivono le vecchie voci
//! `builtin:preset:<preset>@<agente>` in `bot:<bot>-<agente>`, ma non possono
//! creare il Bot derivato (sta in un altro file): lo crea questo passo.
//! Idempotente: un Bot gia' presente non si ricrea e una seconda passata non
//! cambia nulla.

use super::ids::{derived_bot_id, launch_bot_id, parse_derived_bot_id, parse_legacy, LegacyTarget};
use crate::bots::{derived_bot, AgentBot};
use crate::launch_settings::LaunchSettings;
use crate::workspaces::Workspace;

/// Le modifiche da applicare agli store; vuota = niente da fare.
#[derive(Debug, Default, PartialEq)]
pub struct Reconciliation {
    /// `(id del workspace, nuovo avvio predefinito)`.
    pub workspaces: Vec<(String, String)>,
    /// Il nuovo avvio predefinito globale, se cambia.
    pub global_default: Option<String>,
    /// I Bot derivati da aggiungere in fondo all'elenco.
    pub new_bots: Vec<AgentBot>,
}

impl Reconciliation {
    pub fn is_empty(&self) -> bool {
        self.workspaces.is_empty() && self.global_default.is_none() && self.new_bots.is_empty()
    }
}

/// Lo stato dei Bot che la riconciliazione legge.
pub struct BotsContext<'a, F: Fn(&str) -> Option<String>> {
    pub bots: &'a [AgentBot],
    /// Nomi che un Bot non puo' prendere (i segretari), in minuscolo.
    pub taken: &'a [String],
    /// Il nome di un agente registrato; `None` = sconosciuto.
    pub agent_name: F,
}

/// Le modifiche che portano gli avvii predefiniti di `workspaces` e
/// `settings` ai Bot: le vecchie voci si riscrivono, i Bot derivati che mancano
/// si creano (copiati dal predefinito, o dal preset se e' stato cancellato).
/// Una voce derivata con un agente sconosciuto ripiega sul Bot predefinito.
pub fn reconcile<F: Fn(&str) -> Option<String>>(
    workspaces: &[Workspace],
    settings: &LaunchSettings,
    context: &BotsContext<F>,
) -> Reconciliation {
    let mut planner = Planner { context, known: context.bots.to_vec(), new_bots: Vec::new() };
    let workspaces = workspaces
        .iter()
        .filter_map(|ws| Some((ws.id.clone(), planner.visit(ws.default_launch.as_deref()?)?)))
        .collect();
    let global_default = settings.default_launch.as_deref().and_then(|id| planner.visit(id));
    Reconciliation { workspaces, global_default, new_bots: planner.new_bots }
}

struct Planner<'a, F: Fn(&str) -> Option<String>> {
    context: &'a BotsContext<'a, F>,
    /// I Bot esistenti piu' quelli gia' pianificati.
    known: Vec<AgentBot>,
    new_bots: Vec<AgentBot>,
}

impl<F: Fn(&str) -> Option<String>> Planner<'_, F> {
    /// Il nuovo id dell'avvio `id`, se cambia; pianifica il Bot derivato che serve.
    fn visit(&mut self, id: &str) -> Option<String> {
        let target = parse_legacy(id).or_else(|| {
            let (bot, agent) = parse_derived_bot_id(launch_bot_id(id)?)?;
            Some(LegacyTarget::Derived { bot, agent: agent.to_string() })
        })?;
        let resolved = match &target {
            LegacyTarget::Derived { bot, agent } if !self.ensure_derived(bot, agent) => target.base_launch_id(),
            _ => target.launch_id(),
        };
        (resolved != id).then_some(resolved)
    }

    /// Il Bot derivato c'e' (o e' appena stato pianificato); `false` se non si
    /// puo' creare perche' l'agente non e' registrato.
    fn ensure_derived(&mut self, bot: &str, agent: &str) -> bool {
        let id = derived_bot_id(bot, agent);
        if self.known.iter().any(|b| b.id == id) {
            return true;
        }
        let Some(agent_name) = (self.context.agent_name)(agent) else { return false };
        let Some(created) = derived_bot(&self.known, id, bot, agent, &agent_name, self.context.taken) else {
            return false;
        };
        self.known.push(created.clone());
        self.new_bots.push(created);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bots::default_agent_bots;
    use serde_json::json;

    const AGENT: &str = "other";

    fn ws(id: &str, default_launch: Option<&str>) -> Workspace {
        serde_json::from_value(json!({
            "id": id, "name": id, "kind": "windows", "path": "C:\\p", "created_at": 1,
            "default_launch": default_launch,
        }))
        .unwrap()
    }

    fn global(id: Option<&str>) -> LaunchSettings {
        LaunchSettings { default_launch: id.map(str::to_string), show_usage: true }
    }

    fn run(workspaces: &[Workspace], settings: &LaunchSettings, bots: &[AgentBot]) -> Reconciliation {
        let names = |id: &str| (id == AGENT).then(|| "Other".to_string());
        reconcile(workspaces, settings, &BotsContext { bots, taken: &[], agent_name: names })
    }

    /// Applica il piano come lo fa l'avvio dell'app.
    fn apply(
        plan: Reconciliation,
        workspaces: &mut [Workspace],
        settings: &mut LaunchSettings,
        bots: &mut Vec<AgentBot>,
    ) {
        for (id, launch) in plan.workspaces {
            workspaces.iter_mut().find(|w| w.id == id).unwrap().default_launch = Some(launch);
        }
        if let Some(launch) = plan.global_default {
            settings.default_launch = Some(launch);
        }
        bots.extend(plan.new_bots);
    }

    fn ids(bots: &[AgentBot]) -> Vec<&str> {
        bots.iter().map(|b| b.id.as_str()).collect()
    }

    #[test]
    fn a_migrated_derived_launch_gets_its_bot_once() {
        let mut workspaces = vec![ws("a", Some("bot:intake-other")), ws("b", Some("bot:intake-other"))];
        let mut settings = global(Some("bot:skill-wizard-other"));
        let mut bots = default_agent_bots();
        let plan = run(&workspaces, &settings, &bots);
        assert!(plan.workspaces.is_empty() && plan.global_default.is_none(), "{plan:?}");
        assert_eq!(ids(&plan.new_bots), ["intake-other", "skill-wizard-other"]);
        assert_eq!(plan.new_bots[0].name, "Intake (Other)");
        assert_eq!(plan.new_bots[0].agent.as_deref(), Some(AGENT));
        apply(plan, &mut workspaces, &mut settings, &mut bots);
        assert!(run(&workspaces, &settings, &bots).is_empty(), "una seconda passata non cambia nulla");
    }

    #[test]
    fn legacy_ids_read_late_are_rewritten() {
        let workspaces = vec![
            ws("a", Some("builtin:preset:task-intake")),
            ws("b", Some("builtin:preset:project-setup@other")),
            ws("c", Some("builtin:orchestrator")),
            ws("d", Some("builtin:claude-code")),
            ws("e", None),
        ];
        let plan = run(&workspaces, &global(Some("builtin:preset:skill-wizard")), &default_agent_bots());
        assert_eq!(
            plan.workspaces,
            [
                ("a".into(), "bot:intake".into()),
                ("b".into(), "bot:setup-other".into()),
                ("c".into(), "bot:intake".into())
            ]
        );
        assert_eq!(plan.global_default.as_deref(), Some("bot:skill-wizard"));
        assert_eq!(ids(&plan.new_bots), ["setup-other"]);
    }

    #[test]
    fn a_deleted_preset_bot_is_derived_from_the_static_preset() {
        let bots: Vec<_> = default_agent_bots().into_iter().filter(|b| b.id != "intake").collect();
        let plan = run(&[ws("a", Some("bot:intake-other"))], &global(None), &bots);
        assert_eq!(ids(&plan.new_bots), ["intake-other"]);
        assert_eq!(plan.new_bots[0].name, "Intake (Other)");
        assert!(plan.workspaces.is_empty());
    }

    #[test]
    fn an_unknown_agent_falls_back_to_the_preset_bot() {
        let plan = run(&[ws("a", Some("bot:intake-gone"))], &global(Some("builtin:preset:task-intake@gone")), &[]);
        assert_eq!(plan.workspaces, [("a".into(), "bot:intake".into())]);
        assert_eq!(plan.global_default.as_deref(), Some("bot:intake"));
        assert!(plan.new_bots.is_empty());
    }

    #[test]
    fn an_existing_derived_bot_is_never_recreated() {
        let mut bots = default_agent_bots();
        let first = run(&[ws("a", Some("bot:setup-other"))], &global(None), &bots);
        bots.extend(first.new_bots);
        let renamed = bots.last_mut().unwrap();
        renamed.name = "Mine".into();
        let again =
            run(&[ws("a", Some("bot:setup-other"))], &global(Some("builtin:preset:project-setup@other")), &bots);
        assert!(again.new_bots.is_empty() && again.workspaces.is_empty());
        assert_eq!(again.global_default.as_deref(), Some("bot:setup-other"));
    }

    #[test]
    fn other_launches_are_left_alone() {
        let workspaces = [ws("a", Some("shell")), ws("b", Some("ln-1")), ws("c", Some("bot:general"))];
        assert!(run(&workspaces, &global(Some("bot:custom-x")), &default_agent_bots()).is_empty());
    }
}
