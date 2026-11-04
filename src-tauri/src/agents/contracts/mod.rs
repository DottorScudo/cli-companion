//! Contratti (capacita') che il core offre alle integrazioni degli agenti.
//!
//! Il core non sa come funziona un agente: ogni integrazione implementa i
//! contratti che le servono e li dichiara in `AgentIntegration::capabilities`.
//! Un contratto = un file con un trait (piu' i tipi che scambia). Nessun
//! metodo ha un'implementazione di default "non supportato": o l'integrazione
//! dichiara il contratto, o il core non lo usa.

mod automation;
mod capabilities;
mod hook_events;
mod input_profile;
mod integration;
mod package;
mod presets;
mod session_resume;
mod session_usage;
mod usage_limits;
mod user_skills;
mod worker_permissions;

pub use automation::{is_shell_safe_path, AutomationRequest, PermissionMode, WorkerAutomation};
pub use capabilities::{Capabilities, Capability};
pub use hook_events::{HookEvent, HookEventKind, HookEvents, WaitKind};
pub use input_profile::{InputEvent, InputProfile, InputProfileSource, InputRule};
pub use integration::{AgentIntegration, Detection};
pub use package::{plugins_dir, sanitize_env_key, InstallCtx, IntegrationPackage, PackageFile};
pub use presets::PresetLauncher;
pub use session_resume::{ResumeFlag, ResumeSpec, SessionResume};
#[cfg(test)]
pub use session_resume::{COMMAND_PLACEHOLDER, SESSION_ID_PLACEHOLDER};
pub use session_usage::{SessionUsage, SessionUsageReader, TokenTotals};
pub use usage_limits::{LimitWindow, UsageLimits, UsageLimitsReply};
pub use user_skills::UserSkills;
pub use worker_permissions::{PermissionsGuide, RuleFormat, WorkerPermissions};
