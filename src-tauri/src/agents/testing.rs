//! Integrazione finta configurabile, condivisa dai test (avvii, pacchetti,
//! comandi, conformita'). Implementa tutti i contratti con logica semplice e
//! corretta in vocabolario del core; `declares` decide quali dichiarare.

use super::contracts::*;
use super::presets::{Preset, PRESETS};
use crate::platform::{Target, TargetKind};
use crate::validate::{ErrorCode, ValidationError};
use std::path::{Path, PathBuf};

/// Difetto voluto, per provare che la conformita' lo segnala.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flaw {
    /// Il pacchetto non spedisce la skill del primo preset.
    PresetSkillMissing,
    /// Il comando di automazione lascia passare `$` nel prompt.
    AutomationLetsDollarThrough,
    /// Il modello del comando di ripresa non contiene il segnaposto dell'id.
    ResumeTemplateWithoutId,
    /// La regola dei modelli accetta nomi con spazi.
    ModelValidationAcceptsSpaces,
}

#[derive(Debug, Clone)]
pub struct FakeIntegration {
    pub id: &'static str,
    pub interactive: &'static str,
    /// Prefisso del comando dei preset.
    pub preset_cmd: &'static str,
    /// Versione del pacchetto.
    pub version: &'static str,
    pub declares: Vec<Capability>,
    pub flaw: Option<Flaw>,
    /// L'agente risulta presente nel target.
    pub found: bool,
    /// Il pacchetto risulta installato (se l'agente e' presente).
    pub installed: bool,
}

impl FakeIntegration {
    /// Nessun contratto dichiarato.
    pub fn bare(id: &'static str, interactive: &'static str) -> Self {
        Self {
            id,
            interactive,
            preset_cmd: "fake-run",
            version: "1.0-hfake",
            declares: Vec::new(),
            flaw: None,
            found: false,
            installed: false,
        }
    }

    /// Dichiara solo il pacchetto, con la versione data.
    pub fn versioned(id: &'static str, version: &'static str) -> Self {
        Self { version, declares: vec![Capability::Package], ..Self::bare(id, "fake-cli") }
    }

    /// Dichiara solo i preset, con il prefisso di comando dato.
    pub fn presets_only(id: &'static str, interactive: &'static str, preset_cmd: &'static str) -> Self {
        Self { preset_cmd, declares: vec![Capability::Presets], ..Self::bare(id, interactive) }
    }

    /// Dichiara tutti i contratti.
    pub fn full(id: &'static str) -> Self {
        let declares = vec![
            Capability::Package,
            Capability::HookEvents,
            Capability::SessionResume,
            Capability::Automation,
            Capability::WorkerPermissions,
            Capability::Presets,
            Capability::UserSkills,
            Capability::UsageLimits,
            Capability::SessionUsage,
            Capability::InputProfile,
            Capability::Models,
        ];
        Self { declares, ..Self::bare(id, "fake --yes") }
    }

    /// Risultati del rilevamento: se l'agente e' presente e se il pacchetto e' installato.
    pub fn detected(mut self, found: bool, installed: bool) -> Self {
        self.found = found;
        self.installed = installed;
        self
    }

    pub fn with_flaw(mut self, flaw: Flaw) -> Self {
        self.flaw = Some(flaw);
        self
    }
}

impl AgentIntegration for FakeIntegration {
    fn id(&self) -> &'static str {
        self.id
    }

    fn display_name(&self) -> &'static str {
        "Fake"
    }

    fn detect(&self, _: &Target) -> Detection {
        Detection { found: self.found, ..Detection::default() }
    }

    fn interactive_command(&self) -> &'static str {
        self.interactive
    }

    fn capabilities(&self) -> Capabilities<'_> {
        let on = |c| self.declares.contains(&c);
        Capabilities {
            package: on(Capability::Package).then_some(self as &dyn IntegrationPackage),
            hook_events: on(Capability::HookEvents).then_some(self as &dyn HookEvents),
            session_resume: on(Capability::SessionResume).then_some(self as &dyn SessionResume),
            automation: on(Capability::Automation).then_some(self as &dyn WorkerAutomation),
            worker_permissions: on(Capability::WorkerPermissions).then_some(self as &dyn WorkerPermissions),
            presets: on(Capability::Presets).then_some(self as &dyn PresetLauncher),
            user_skills: on(Capability::UserSkills).then_some(self as &dyn UserSkills),
            usage_limits: on(Capability::UsageLimits).then_some(self as &dyn UsageLimits),
            session_usage: on(Capability::SessionUsage).then_some(self as &dyn SessionUsageReader),
            input_profile: on(Capability::InputProfile).then_some(self as &dyn InputProfileSource),
            models: on(Capability::Models).then_some(self as &dyn ModelCatalog),
        }
    }
}

impl IntegrationPackage for FakeIntegration {
    fn version(&self, _: &InstallCtx, _: &Target) -> Option<String> {
        Some(self.version.to_string())
    }

    fn files(&self, _: &InstallCtx, _: &Target) -> Result<Vec<PackageFile>, String> {
        let manifest =
            PackageFile { path: "manifest.json".into(), content: format!("{{\"version\":\"{}\"}}", self.version) };
        let skipped = usize::from(self.flaw == Some(Flaw::PresetSkillMissing));
        let skills = PRESETS
            .iter()
            .skip(skipped)
            .map(|p| PackageFile { path: PresetLauncher::skill_path(self, p), content: p.skill_text() });
        Ok(std::iter::once(manifest).chain(skills).collect())
    }

    fn is_installed(&self, _: &Target, _: &Detection) -> bool {
        self.installed
    }

    fn install(&self, _: &InstallCtx, _: &Target, _: &Detection) -> Result<String, String> {
        Ok(String::new())
    }

    fn uninstall(&self, _: &InstallCtx, _: &Target, _: &Detection) -> Result<String, String> {
        Ok(String::new())
    }
}

/// Stdin finto di un hook che aspetta: `wait:<nome>`, con nomi propri
/// dell'agente finto (diversi da quelli di qualunque agente reale).
pub const FAKE_WAIT_PREFIX: &str = "wait:";
pub const FAKE_WAIT_APPROVAL: &str = "tool-approval";
pub const FAKE_WAIT_NUDGE: &str = "still-there";
pub const FAKE_WAIT_ASK: &str = "user-answer";

impl HookEvents for FakeIntegration {
    /// Solo il tipo di attesa, da `wait:<nome>`; un nome sconosciuto e'
    /// un'attesa generica, ogni altro stdin un evento senza dettagli.
    fn normalize(&self, _: HookEventKind, raw: &str) -> HookEvent {
        let wait = raw.strip_prefix(FAKE_WAIT_PREFIX).map(|name| match name {
            FAKE_WAIT_APPROVAL => WaitKind::Permission,
            FAKE_WAIT_NUDGE => WaitKind::Idle,
            FAKE_WAIT_ASK => WaitKind::Question,
            _ => WaitKind::Other,
        });
        HookEvent { wait, ..HookEvent::default() }
    }

    fn stop_block_output(&self, reason: &str) -> String {
        format!("fake-stop {reason}")
    }
}

impl SessionResume for FakeIntegration {
    fn resume_spec(&self) -> ResumeSpec {
        let program = self.interactive.split_whitespace().next().unwrap_or_default();
        let resume_template = if self.flaw == Some(Flaw::ResumeTemplateWithoutId) {
            "{command} --resume"
        } else {
            "{command} --resume {id}"
        };
        ResumeSpec { command_names: vec![program], strip_flags: Vec::new(), resume_template }
    }
}

impl WorkerAutomation for FakeIntegration {
    fn command(&self, _: TargetKind, req: &AutomationRequest) -> Option<String> {
        let leaks_dollar = self.flaw == Some(Flaw::AutomationLetsDollarThrough);
        let unsafe_char =
            |c: char| matches!(c, '"' | '`' | '%' | '!' | '\\') || c.is_control() || (c == '$' && !leaks_dollar);
        if req.bootstrap_prompt.is_empty() || req.bootstrap_prompt.chars().any(unsafe_char) {
            return None;
        }
        let mut cmd = format!("{} \"{}\"", self.id, req.bootstrap_prompt);
        if let Some(file) = req.permissions_file {
            if !crate::agents::is_shell_safe_path(file) {
                return None;
            }
            cmd.push_str(&format!(" --rules \"{file}\""));
        }
        if let Some(model) = req.model {
            let valid = !model.is_empty()
                && model.len() <= 64
                && model.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
            if !valid {
                return None;
            }
            cmd.push_str(&format!(" --model {model}"));
        }
        Some(cmd)
    }
}

impl WorkerPermissions for FakeIntegration {
    fn rule_format(&self) -> RuleFormat {
        RuleFormat { max_rules: 5, max_rule_chars: 20, example: "fake-rule" }
    }

    fn guide(&self) -> PermissionsGuide {
        PermissionsGuide {
            rules_help: "Regole finte.",
            bypass_note: "Nota finta.",
            allow_placeholder: "fake-allow",
            deny_placeholder: "fake-deny",
        }
    }

    /// Una regola e' una parola senza spazi.
    fn validate_rules(&self, rules: &[String]) -> Result<Vec<String>, ValidationError> {
        let mut out: Vec<String> = Vec::new();
        for rule in rules.iter().map(|r| r.trim()).filter(|r| !r.is_empty()) {
            if rule.contains(char::is_whitespace) {
                return Err(ValidationError::new(ErrorCode::PermissionRuleBadShape).value(rule.to_string()));
            }
            if !out.iter().any(|r| r == rule) {
                out.push(rule.to_string());
            }
        }
        Ok(out)
    }

    fn prepare(
        &self,
        _: &Path,
        workspace_id: &str,
        _: TargetKind,
        allow: &[String],
        deny: &[String],
    ) -> Result<Option<String>, String> {
        let any = !allow.is_empty() || !deny.is_empty();
        Ok(any.then(|| format!("rules/{workspace_id}.json")))
    }
}

impl PresetLauncher for FakeIntegration {
    fn command(&self, preset: &Preset) -> String {
        format!("{} {}", self.preset_cmd, preset.id)
    }

    fn skill_path(&self, preset: &Preset) -> String {
        format!("skills/{}/SKILL.md", preset.id)
    }
}

impl UserSkills for FakeIntegration {
    fn windows_dir(&self) -> Result<PathBuf, String> {
        Ok(PathBuf::from("C:/fake/skills"))
    }

    fn wsl_home_subpath(&self) -> &'static str {
        ".fake/skills"
    }
}

impl UsageLimits for FakeIntegration {
    fn limits(&self, _: &Target) -> UsageLimitsReply {
        UsageLimitsReply::status("unavailable", Some("finto"))
    }

    fn notice(&self) -> &'static str {
        "Avviso finto."
    }
}

impl SessionUsageReader for FakeIntegration {
    fn session_usage(&self, _: &Target, transcript_path: &str) -> Result<SessionUsage, String> {
        if transcript_path.is_empty() || transcript_path.contains("..") {
            return Err("percorso non valido".to_string());
        }
        Ok(SessionUsage::default())
    }
}

impl InputProfileSource for FakeIntegration {
    fn input_profile(&self) -> InputProfile {
        InputProfile {
            rules: vec![InputRule { event: InputEvent::ImagePaste, env_kind: None, send: "\x1bv".to_string() }],
        }
    }
}

/// Modelli dell'agente finto, con nomi che non appartengono a nessun agente reale.
pub const FAKE_MODEL_SMALL: &str = "fake-small";
pub const FAKE_MODEL_LARGE: &str = "fake-large";
const FAKE_MODEL_PREFIX: &str = "fake-";

const FAKE_SUGGESTED: &[SuggestedModel] = &[
    SuggestedModel { id: FAKE_MODEL_SMALL, label: "Fake small", tier: ModelTier::Standard },
    SuggestedModel { id: FAKE_MODEL_LARGE, label: "Fake large", tier: ModelTier::Strong },
];

impl ModelCatalog for FakeIntegration {
    fn suggested(&self) -> &'static [SuggestedModel] {
        FAKE_SUGGESTED
    }

    fn default_model(&self) -> Option<&'static str> {
        Some(FAKE_MODEL_SMALL)
    }

    /// Prefisso `fake-` e solo `[a-z0-9-]`, al massimo `MODEL_MAX` byte. Col
    /// difetto accetta qualunque nome non vuoto, anche con spazi.
    fn is_valid(&self, model: &str) -> bool {
        let lenient = self.flaw == Some(Flaw::ModelValidationAcceptsSpaces);
        (lenient || model.starts_with(FAKE_MODEL_PREFIX))
            && !model.is_empty()
            && model.len() <= crate::validate::MODEL_MAX
            && model.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || (lenient && c == ' '))
    }
}
