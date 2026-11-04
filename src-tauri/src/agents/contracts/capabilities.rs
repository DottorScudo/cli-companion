//! L'elenco dei contratti dichiarati da un'integrazione.

use super::{
    HookEvents, InputProfileSource, IntegrationPackage, ModelCatalog, PresetLauncher, SessionResume,
    SessionUsageReader, UsageLimits, UserSkills, WorkerAutomation, WorkerPermissions,
};
use serde::Serialize;
use ts_rs::TS;

/// Contratti implementati da un'integrazione (`None` = non li dichiara).
/// Niente `Default`: ogni integrazione elenca tutti i campi, quindi un nuovo
/// contratto la obbliga a decidere.
#[derive(Clone, Copy)]
pub struct Capabilities<'a> {
    pub package: Option<&'a dyn IntegrationPackage>,
    pub hook_events: Option<&'a dyn HookEvents>,
    pub session_resume: Option<&'a dyn SessionResume>,
    pub automation: Option<&'a dyn WorkerAutomation>,
    pub worker_permissions: Option<&'a dyn WorkerPermissions>,
    pub presets: Option<&'a dyn PresetLauncher>,
    pub user_skills: Option<&'a dyn UserSkills>,
    pub usage_limits: Option<&'a dyn UsageLimits>,
    pub session_usage: Option<&'a dyn SessionUsageReader>,
    pub input_profile: Option<&'a dyn InputProfileSource>,
    pub models: Option<&'a dyn ModelCatalog>,
}

/// Nome di un contratto, come lo vede il frontend (kebab-case).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum Capability {
    Package,
    HookEvents,
    SessionResume,
    Automation,
    WorkerPermissions,
    Presets,
    UserSkills,
    UsageLimits,
    SessionUsage,
    InputProfile,
    Models,
}

impl Capabilities<'_> {
    /// Contratti dichiarati, nell'ordine dei campi. La destrutturazione e'
    /// esaustiva (niente `..`): un nuovo campo non compila finche' non e' qui.
    pub fn declared(&self) -> Vec<Capability> {
        let Capabilities {
            package,
            hook_events,
            session_resume,
            automation,
            worker_permissions,
            presets,
            user_skills,
            usage_limits,
            session_usage,
            input_profile,
            models,
        } = self;
        [
            (package.is_some(), Capability::Package),
            (hook_events.is_some(), Capability::HookEvents),
            (session_resume.is_some(), Capability::SessionResume),
            (automation.is_some(), Capability::Automation),
            (worker_permissions.is_some(), Capability::WorkerPermissions),
            (presets.is_some(), Capability::Presets),
            (user_skills.is_some(), Capability::UserSkills),
            (usage_limits.is_some(), Capability::UsageLimits),
            (session_usage.is_some(), Capability::SessionUsage),
            (input_profile.is_some(), Capability::InputProfile),
            (models.is_some(), Capability::Models),
        ]
        .into_iter()
        .filter_map(|(on, capability)| on.then_some(capability))
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_serialize_kebab_case() {
        let json = |c| serde_json::to_value(c).unwrap();
        assert_eq!(json(Capability::Package), "package");
        assert_eq!(json(Capability::HookEvents), "hook-events");
        assert_eq!(json(Capability::SessionResume), "session-resume");
        assert_eq!(json(Capability::Automation), "automation");
        assert_eq!(json(Capability::WorkerPermissions), "worker-permissions");
        assert_eq!(json(Capability::Presets), "presets");
        assert_eq!(json(Capability::UserSkills), "user-skills");
        assert_eq!(json(Capability::UsageLimits), "usage-limits");
        assert_eq!(json(Capability::SessionUsage), "session-usage");
        assert_eq!(json(Capability::InputProfile), "input-profile");
        assert_eq!(json(Capability::Models), "models");
    }

    #[test]
    fn declared_is_empty_without_contracts() {
        let none = Capabilities {
            package: None,
            hook_events: None,
            session_resume: None,
            automation: None,
            worker_permissions: None,
            presets: None,
            user_skills: None,
            usage_limits: None,
            session_usage: None,
            input_profile: None,
            models: None,
        };
        assert!(none.declared().is_empty());
    }
}
