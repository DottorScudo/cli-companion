//! Suite di conformita': ogni integrazione registrata deve rispettare i
//! contratti che dichiara, senza che nessuno debba ricordarsi di testarla.
//! `violations` ritorna le violazioni come testo, cosi' la suite si prova
//! anche contro un'integrazione volutamente rotta.

use super::contracts::*;
use super::presets::PRESETS;
use super::registry;
use super::testing::{FakeIntegration, Flaw};
use crate::platform::{Target, TargetKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Cartella temporanea unica (pid + contatore + nome), eliminata al drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("mc-conformance-{}-{n}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Riga di comando digitata in PowerShell, cmd e bash: niente caratteri di
/// controllo, niente `$`, backtick, `%`, `!`, virgolette bilanciate.
fn shell_safe(command: &str) -> bool {
    !command.chars().any(|c| c.is_control() || matches!(c, '$' | '`' | '%' | '!'))
        && command.matches('"').count().is_multiple_of(2)
}

fn core(i: &dyn AgentIntegration) -> Vec<String> {
    let mut v = Vec::new();
    let id = i.id();
    let id_ok = !id.is_empty()
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-');
    if !id_ok {
        v.push(format!("id non valido: {id:?} (solo [a-z0-9-], senza trattini ai bordi)"));
    }
    if i.display_name().trim().is_empty() {
        v.push("display_name vuoto".to_string());
    }
    let interactive = i.interactive_command();
    if interactive.trim().is_empty() || !shell_safe(interactive) {
        v.push(format!("interactive_command non valido o non sicuro per la shell: {interactive:?}"));
    }
    let caps = i.capabilities();
    let present = [
        (caps.package.is_some(), Capability::Package),
        (caps.hook_events.is_some(), Capability::HookEvents),
        (caps.session_resume.is_some(), Capability::SessionResume),
        (caps.automation.is_some(), Capability::Automation),
        (caps.worker_permissions.is_some(), Capability::WorkerPermissions),
        (caps.presets.is_some(), Capability::Presets),
        (caps.user_skills.is_some(), Capability::UserSkills),
        (caps.usage_limits.is_some(), Capability::UsageLimits),
        (caps.session_usage.is_some(), Capability::SessionUsage),
        (caps.input_profile.is_some(), Capability::InputProfile),
    ];
    let expected: Vec<_> = present.into_iter().filter_map(|(on, c)| on.then_some(c)).collect();
    if caps.declared() != expected {
        v.push(format!("declared() {:?} non coincide con i contratti presenti {expected:?}", caps.declared()));
    }
    v
}

fn install_ctx(data_dir: &Path) -> InstallCtx {
    InstallCtx { exe: "C:/Program Files/cli-companion/cli-companion.exe".to_string(), data_dir: data_dir.to_path_buf() }
}

fn conformance_targets() -> [Target; 2] {
    [Target::Windows, Target::Wsl { distro: "Ubuntu-24.04".to_string() }]
}

fn package(package: &dyn IntegrationPackage, data_dir: &Path) -> Vec<String> {
    let mut v = Vec::new();
    let ctx = install_ctx(data_dir);
    for target in conformance_targets() {
        let key = target.key();
        match package.version(&ctx, &target) {
            None => v.push(format!("package: version() None per {key}")),
            Some(version) => {
                if version.is_empty() || version.chars().any(char::is_whitespace) {
                    v.push(format!("package: version() vuota o con spazi per {key}: {version:?}"));
                }
                if package.version(&ctx, &target) != Some(version) {
                    v.push(format!("package: version() non stabile per {key}"));
                }
            }
        }
        match package.files(&ctx, &target) {
            Err(e) => v.push(format!("package: files() fallisce per {key}: {e}")),
            Ok(files) => {
                for (n, f) in files.iter().enumerate() {
                    let p = f.path.as_str();
                    let drive = p.as_bytes().get(1) == Some(&b':');
                    if p.is_empty() || p.starts_with('/') || drive {
                        v.push(format!("package: path vuoto o non relativo: {p:?}"));
                    }
                    if p.contains('\\') {
                        v.push(format!("package: path con backslash: {p:?}"));
                    }
                    if p.split('/').any(|c| c == "..") {
                        v.push(format!("package: path con '..': {p:?}"));
                    }
                    if files[..n].iter().any(|other| other.path == f.path) {
                        v.push(format!("package: path duplicato: {p:?}"));
                    }
                    if f.content.starts_with('\u{feff}') {
                        v.push(format!("package: BOM in {p:?}"));
                    }
                }
            }
        }
    }
    v
}

fn presets(i: &dyn AgentIntegration, launcher: &dyn PresetLauncher, data_dir: &Path) -> Vec<String> {
    let mut v = Vec::new();
    let Some(pkg) = i.capabilities().package else {
        return vec!["presets: richiede il contratto package (la skill va spedita)".to_string()];
    };
    let files = pkg.files(&install_ctx(data_dir), &Target::Windows).unwrap_or_default();
    let mut seen: Vec<String> = Vec::new();
    for preset in PRESETS {
        let command = launcher.command(preset);
        if command.trim().is_empty() || !shell_safe(&command) {
            v.push(format!("presets: comando non valido o non sicuro per {}: {command:?}", preset.id));
        }
        let path = launcher.skill_path(preset);
        if seen.contains(&path) {
            v.push(format!("presets: skill_path duplicato: {path:?}"));
        }
        match files.iter().find(|f| f.path == path) {
            None => v.push(format!("presets: la skill del preset {} manca nel pacchetto ({path})", preset.id)),
            Some(f) if f.content != preset.skill_text() => {
                v.push(format!("presets: la skill del preset {} nel pacchetto non coincide col catalogo", preset.id));
            }
            Some(_) => {}
        }
        seen.push(path);
    }
    v
}

fn request<'a>(prompt: &'a str, model: Option<&'a str>, permissions_file: Option<&'a str>) -> AutomationRequest<'a> {
    AutomationRequest { bootstrap_prompt: prompt, permission_mode: PermissionMode::Default, model, permissions_file }
}

fn automation(automation: &dyn WorkerAutomation) -> Vec<String> {
    let mut v = Vec::new();
    let prompt = crate::automation::BOOTSTRAP_PROMPT;
    for kind in [TargetKind::Windows, TargetKind::Wsl] {
        for mode in PermissionMode::ALL {
            let req = AutomationRequest { permission_mode: mode, ..request(prompt, None, None) };
            match automation.command(kind, &req) {
                None => v.push(format!("automation: nessun comando per {kind:?}/{mode:?}")),
                Some(cmd) => {
                    if !cmd.contains(prompt) {
                        v.push(format!("automation: il comando non contiene il prompt ({kind:?}/{mode:?})"));
                    }
                    if !shell_safe(&cmd) {
                        v.push(format!("automation: comando non sicuro per la shell: {cmd:?}"));
                    }
                }
            }
        }
        let run = |req: &AutomationRequest| automation.command(kind, req);
        for c in ['"', '$', '`', '%', '!', '\\', '\n'] {
            let bad = format!("a{c}b");
            if run(&request(&bad, None, None)).is_some() {
                v.push(format!("automation: prompt con {c:?} accettato"));
            }
        }
        if run(&request("", None, None)).is_some() {
            v.push("automation: prompt vuoto accettato".to_string());
        }
        if run(&request(prompt, Some("claude-opus-4.1"), None)).is_none() {
            v.push("automation: modello valido rifiutato".to_string());
        }
        let long = "m".repeat(65);
        for model in ["", "m x", "m;ls", "m\"", long.as_str()] {
            if run(&request(prompt, Some(model), None)).is_some() {
                v.push(format!("automation: modello non valido accettato: {model:?}"));
            }
        }
        for file in ["C:/a$b/x.json", "C:/a`b/x.json", "C:/100%/x.json", "C:/a\"b/x.json", "C:/hi!/x.json"] {
            if run(&request(prompt, None, Some(file))).is_some() {
                v.push(format!("automation: permissions_file non sicuro accettato: {file:?}"));
            }
        }
        let safe = "C:/data/worker-permissions/ws-1.json";
        match run(&request(prompt, None, Some(safe))) {
            Some(cmd) if cmd.contains(safe) => {}
            other => v.push(format!("automation: permissions_file sicuro non usato: {other:?}")),
        }
    }
    v
}

fn input_profile(source: &dyn InputProfileSource) -> Vec<String> {
    let mut v = Vec::new();
    let rules = source.input_profile().rules;
    for (n, rule) in rules.iter().enumerate() {
        if rule.send.is_empty() {
            v.push(format!("input_profile: regola {:?}/{:?} con send vuoto", rule.event, rule.env_kind));
        }
        if rules[..n].iter().any(|other| other.event == rule.event && other.env_kind == rule.env_kind) {
            v.push(format!("input_profile: regola duplicata {:?}/{:?}", rule.event, rule.env_kind));
        }
    }
    v
}

/// Nome di programma (o flag) utilizzabile cosi' com'e' in una riga di comando.
fn plain_token(token: &str) -> bool {
    !token.is_empty() && !token.contains(char::is_whitespace) && shell_safe(token) && !token.contains('"')
}

fn session_resume(i: &dyn AgentIntegration, resume: &dyn SessionResume) -> Vec<String> {
    let mut v = Vec::new();
    let spec = resume.resume_spec();
    if spec.command_names.is_empty() {
        v.push("session_resume: command_names vuoto".to_string());
    }
    for name in &spec.command_names {
        if !plain_token(name) || *name != name.to_lowercase() {
            v.push(format!("session_resume: nome di comando non valido (minuscolo, senza spazi): {name:?}"));
        }
    }
    let program = i.interactive_command().split_whitespace().next().unwrap_or_default().to_lowercase();
    if !spec.command_names.iter().any(|name| *name == program) {
        v.push(format!("session_resume: il comando interattivo ({program:?}) non e' tra i command_names"));
    }
    for flag in &spec.strip_flags {
        if flag.names.is_empty() {
            v.push("session_resume: flag senza nomi".to_string());
        }
        for name in &flag.names {
            if !plain_token(name) || !name.starts_with('-') {
                v.push(format!("session_resume: nome di flag non valido (deve iniziare con '-'): {name:?}"));
            }
        }
    }
    let template = spec.resume_template;
    for placeholder in [COMMAND_PLACEHOLDER, SESSION_ID_PLACEHOLDER] {
        let found = template.matches(placeholder).count();
        if found != 1 {
            v.push(format!(
                "session_resume: il modello deve contenere {placeholder} una volta, ne ha {found}: {template:?}"
            ));
        }
    }
    let canonical = spec.command_names.first().copied().unwrap_or_default();
    let resumed = template.replace(COMMAND_PLACEHOLDER, canonical).replace(SESSION_ID_PLACEHOLDER, "abc-123");
    if !shell_safe(&resumed) {
        v.push(format!("session_resume: comando di ripresa non sicuro per la shell: {resumed:?}"));
    }
    v
}

fn usage_limits(limits: &dyn UsageLimits) -> Vec<String> {
    if limits.notice().trim().is_empty() {
        vec!["usage_limits: notice vuoto".to_string()]
    } else {
        Vec::new()
    }
}

fn hook_events(events: &dyn HookEvents) -> Vec<String> {
    let mut v = Vec::new();
    for kind in HookEventKind::ALL {
        for raw in ["", "not json", "[]", "{}"] {
            if events.normalize(kind, raw) != HookEvent::default() {
                v.push(format!("hook_events: {kind:?} con {raw:?} non da' un evento senza dettagli"));
            }
        }
    }
    let line = events.stop_block_output("x");
    if line.is_empty() || !line.contains('x') {
        v.push(format!("hook_events: stop_block_output(\"x\") vuoto o senza il motivo: {line:?}"));
    }
    v
}

fn worker_permissions(permissions: &dyn WorkerPermissions, data_dir: &Path) -> Vec<String> {
    let mut v = Vec::new();
    let format = permissions.rule_format();
    if format.max_rules == 0 || format.max_rule_chars == 0 || format.example.trim().is_empty() {
        v.push(format!("worker_permissions: forma delle regole incompleta: {format:?}"));
    }
    let guide = permissions.guide();
    if guide.rules_help.trim().is_empty() || guide.bypass_note.trim().is_empty() {
        v.push("worker_permissions: guida vuota".to_string());
    }
    let blank = ["".to_string(), "  ".to_string()];
    match permissions.validate_rules(&blank) {
        Ok(rules) if rules.is_empty() => {}
        other => v.push(format!("worker_permissions: righe vuote attese scartate, ottenuto {other:?}")),
    }
    match permissions.validate_rules(&[format.example.to_string()]) {
        Ok(rules) if rules == [format.example] => {}
        other => {
            v.push(format!("worker_permissions: l'esempio {:?} non e' una regola valida: {other:?}", format.example))
        }
    }
    match permissions.prepare(data_dir, "ws-conformance", TargetKind::Windows, &[], &[]) {
        Ok(None) => {}
        other => v.push(format!("worker_permissions: senza regole atteso Ok(None), ottenuto {other:?}")),
    }
    v
}

fn user_skills(skills: &dyn UserSkills) -> Vec<String> {
    let mut v = Vec::new();
    let sub = skills.wsl_home_subpath();
    let drive = sub.as_bytes().get(1) == Some(&b':');
    if sub.is_empty() || sub.starts_with('/') || drive || sub.contains('\\') || sub.split('/').any(|c| c == "..") {
        v.push(format!("user_skills: wsl_home_subpath non valido: {sub:?}"));
    }
    v
}

fn session_usage(reader: &dyn SessionUsageReader) -> Vec<String> {
    ["", "../x"]
        .into_iter()
        .filter(|path| reader.session_usage(&Target::Windows, path).is_ok())
        .map(|path| format!("session_usage: il path {path:?} dovrebbe essere rifiutato"))
        .collect()
}

/// Tutte le violazioni dei contratti dichiarati da `i` (vuoto = conforme).
fn violations(i: &dyn AgentIntegration) -> Vec<String> {
    let scratch = Scratch::new(i.id());
    let dir = scratch.0.as_path();
    let caps = i.capabilities();
    let mut v = core(i);
    if let Some(c) = caps.package {
        v.extend(package(c, dir));
    }
    if let Some(c) = caps.presets {
        v.extend(presets(i, c, dir));
    }
    if let Some(c) = caps.automation {
        v.extend(automation(c));
    }
    if let Some(c) = caps.input_profile {
        v.extend(input_profile(c));
    }
    if let Some(c) = caps.session_resume {
        v.extend(session_resume(i, c));
    }
    if let Some(c) = caps.hook_events {
        v.extend(hook_events(c));
    }
    if let Some(c) = caps.worker_permissions {
        v.extend(worker_permissions(c, dir));
    }
    if let Some(c) = caps.user_skills {
        v.extend(user_skills(c));
    }
    if let Some(c) = caps.session_usage {
        v.extend(session_usage(c));
    }
    if let Some(c) = caps.usage_limits {
        v.extend(usage_limits(c));
    }
    v.into_iter().map(|m| format!("{}: {m}", i.id())).collect()
}

#[test]
fn every_registered_integration_conforms() {
    for integration in registry::all() {
        let found = violations(*integration);
        assert!(found.is_empty(), "{}", found.join("\n"));
    }
}

#[test]
fn fake_integrations_conform() {
    for fake in [FakeIntegration::full("fake-full"), FakeIntegration::bare("fake-bare", "fake --yes")] {
        let found = violations(&fake);
        assert!(found.is_empty(), "{}", found.join("\n"));
    }
}

#[test]
fn a_missing_preset_skill_is_reported() {
    let broken = FakeIntegration::full("broken-skill").with_flaw(Flaw::PresetSkillMissing);
    let found = violations(&broken);
    assert!(found.iter().any(|m| m.contains("presets:") && m.contains("manca nel pacchetto")), "{found:?}");
}

#[test]
fn an_unsafe_automation_command_is_reported() {
    let broken = FakeIntegration::full("broken-dollar").with_flaw(Flaw::AutomationLetsDollarThrough);
    let found = violations(&broken);
    assert!(found.iter().any(|m| m.contains("automation:") && m.contains("'$'")), "{found:?}");
}

#[test]
fn a_resume_template_without_the_session_id_is_reported() {
    let broken = FakeIntegration::full("broken-resume").with_flaw(Flaw::ResumeTemplateWithoutId);
    let found = violations(&broken);
    assert!(found.iter().any(|m| m.contains("session_resume:") && m.contains(SESSION_ID_PLACEHOLDER)), "{found:?}");
}

#[test]
fn identity_and_declaration_problems_are_reported() {
    let found = violations(&FakeIntegration::bare("Bad Id-", "echo $HOME"));
    assert!(found.iter().any(|m| m.contains("id non valido")), "{found:?}");
    assert!(found.iter().any(|m| m.contains("interactive_command")), "{found:?}");
    // Preset senza pacchetto: la skill non avrebbe dove stare.
    let presets_only = FakeIntegration::presets_only("only-presets", "fake --yes", "fake-run");
    assert!(violations(&presets_only).iter().any(|m| m.contains("richiede il contratto package")));
}

#[test]
fn registry_ids_are_unique_and_the_default_worker_agent_can_automate() {
    let ids: Vec<_> = registry::all().iter().map(|i| i.id()).collect();
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), ids.len(), "id duplicati: {ids:?}");
    let default = registry::find(registry::default_worker_integration().id()).expect("agente dei worker registrato");
    assert!(default.capabilities().automation.is_some(), "l'agente dei worker deve dichiarare automation");
}
