//! Contratto `IntegrationPackage` di Claude Code: il plugin locale (generato da
//! `plugin.rs`) installato con `claude plugin ...`.

use super::plugin::{plugin_files, plugin_version_for};
use super::{ClaudeCode, AGENT_ID, MARKETPLACE, PLUGIN, PLUGIN_ID};
use crate::agents::detection::PLUGIN_LIST_TIMEOUT;
use crate::agents::presets::PRESETS;
use crate::agents::{Detection, InstallCtx, IntegrationPackage, PackageFile};
use crate::platform::Target;
use std::path::Path;

/// Toglie da `skills/` le cartelle che non sono piu' preset (es. le vecchie
/// `orchestrate` e `workflow`): la reinstallazione non deve lasciarle nel plugin.
fn remove_obsolete_skills(plugin_dir: &Path) -> std::io::Result<()> {
    let skills = plugin_dir.join("skills");
    let entries = match std::fs::read_dir(&skills) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries.filter_map(Result::ok) {
        let current = entry.file_name().to_str().is_some_and(|n| PRESETS.iter().any(|p| p.id == n));
        if !current {
            let path = entry.path();
            if path.is_dir() {
                std::fs::remove_dir_all(path)?;
            } else {
                std::fs::remove_file(path)?;
            }
        }
    }
    Ok(())
}

/// `true` se l'output di `claude plugin list --json` contiene il nostro
/// plugin (match esatto su `id`). Tollera un array o un oggetto con `plugins`,
/// e rumore prima/dopo il JSON.
fn plugin_listed(list_json: &str, id: &str) -> bool {
    let parse = |s: &str| serde_json::from_str::<serde_json::Value>(s).ok();
    let value = parse(list_json).or_else(|| {
        let start = list_json.find(['[', '{'])?;
        let end = list_json.rfind([']', '}'])?;
        parse(list_json.get(start..=end)?)
    });
    let Some(value) = value else {
        return false;
    };
    let list = match &value {
        serde_json::Value::Array(a) => Some(a),
        v => v.get("plugins").and_then(|p| p.as_array()),
    };
    list.is_some_and(|a| a.iter().any(|p| p.get("id").and_then(|v| v.as_str()) == Some(id)))
}

/// Esegue un passo del flusso e lo registra nel log; in caso di errore
/// ritorna il messaggio con il log accumulato.
fn step(log: &mut Vec<String>, target: &Target, program: &str, label: &str, args: &[&str]) -> Result<(), String> {
    match target.run(program, args) {
        Ok(_) => {
            log.push(format!("{label}: ok"));
            Ok(())
        }
        Err(e) => Err(format!("{label} fallita: {e}\n{}", log.join("\n"))),
    }
}

/// Come `step` ma gli errori non fermano il flusso (pulizia best-effort).
fn step_lenient(log: &mut Vec<String>, target: &Target, program: &str, label: &str, args: &[&str]) {
    match target.run(program, args) {
        Ok(_) => log.push(format!("{label}: ok")),
        Err(e) => log.push(format!("{label}: {}", e.lines().next().unwrap_or(""))),
    }
}

impl IntegrationPackage for ClaudeCode {
    fn version(&self, ctx: &InstallCtx, target: &Target) -> Option<String> {
        ctx.exe_for(target).ok().map(|exe| plugin_version_for(&exe))
    }

    fn files(&self, ctx: &InstallCtx, target: &Target) -> Result<Vec<PackageFile>, String> {
        let exe = ctx.exe_for(target)?;
        Ok(plugin_files(&exe).into_iter().map(|(path, content)| PackageFile { path, content }).collect())
    }

    fn is_installed(&self, target: &Target, det: &Detection) -> bool {
        let Some(program) = det.path.as_deref() else {
            return false;
        };
        target
            .run_timeout(program, &["plugin", "list", "--json"], PLUGIN_LIST_TIMEOUT)
            .map(|out| plugin_listed(&out, PLUGIN_ID))
            .unwrap_or(false)
    }

    fn install(&self, ctx: &InstallCtx, target: &Target, det: &Detection) -> Result<String, String> {
        let program = det
            .path
            .as_deref()
            .filter(|_| det.found)
            .ok_or_else(|| "Claude Code non trovato in questo env".to_string())?;
        let root = ctx.plugin_root(AGENT_ID, target);
        let files = self.files(ctx, target)?;
        remove_obsolete_skills(&root.join("plugins").join(PLUGIN))
            .map_err(|e| format!("pulizia skill obsolete: {e}"))?;
        for PackageFile { path, content } in files {
            let file = root.join(&path);
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("scrittura plugin: {e}"))?;
            }
            std::fs::write(&file, content).map_err(|e| format!("scrittura plugin: {e}"))?;
        }
        let market = target.path_arg(&root)?;
        let plugin_dir = format!("{market}/plugins/{PLUGIN}");
        let mut log = vec![format!("plugin scritto in {}", root.display())];

        // Valida prima di installare: un errore qui e' un nostro bug.
        step(&mut log, target, program, "validazione plugin", &["plugin", "validate", &plugin_dir])?;
        step(&mut log, target, program, "validazione marketplace", &["plugin", "validate", &market])?;

        // Reinstallazione deterministica: pulizia (errori ignorati, la prima
        // volta non c'e' nulla da togliere), poi aggiunta e installazione.
        step_lenient(
            &mut log,
            target,
            program,
            "pulizia plugin precedente",
            &["plugin", "uninstall", PLUGIN_ID, "--scope", "user"],
        );
        step_lenient(
            &mut log,
            target,
            program,
            "pulizia marketplace precedente",
            &["plugin", "marketplace", "remove", MARKETPLACE],
        );
        step(&mut log, target, program, "aggiunta marketplace", &["plugin", "marketplace", "add", &market])?;
        step(&mut log, target, program, "installazione plugin", &["plugin", "install", PLUGIN_ID, "--scope", "user"])?;

        log.push(if self.is_installed(target, det) {
            "stato: installato".to_string()
        } else {
            "stato: NON rilevato in 'plugin list'".to_string()
        });
        Ok(log.join("\n"))
    }

    fn uninstall(&self, ctx: &InstallCtx, target: &Target, det: &Detection) -> Result<String, String> {
        let mut log = Vec::new();
        if let Some(program) = det.path.as_deref().filter(|_| det.found) {
            step_lenient(
                &mut log,
                target,
                program,
                "rimozione plugin",
                &["plugin", "uninstall", PLUGIN_ID, "--scope", "user"],
            );
            step_lenient(
                &mut log,
                target,
                program,
                "rimozione marketplace",
                &["plugin", "marketplace", "remove", MARKETPLACE],
            );
        } else {
            log.push("Claude Code non trovato: rimossi solo i file generati".to_string());
        }
        let root = ctx.plugin_root(AGENT_ID, target);
        match std::fs::remove_dir_all(&root) {
            Ok(()) => log.push("file generati eliminati".to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => log.push(format!("file generati: {e}")),
        }
        Ok(log.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::detection::resolve_wsl_command;

    #[test]
    fn files_are_the_generated_plugin_for_the_target_exe() {
        let ctx = InstallCtx { exe: "C:/a/cli-companion.exe".to_string(), data_dir: "unused".into() };
        let win = ClaudeCode.files(&ctx, &Target::Windows).unwrap();
        assert!(win.iter().any(|f| f.path == ".claude-plugin/marketplace.json"));
        assert!(win
            .iter()
            .any(|f| f.path.ends_with("hooks/hooks.json") && f.content.contains("C:/a/cli-companion.exe")));
        let wsl = ClaudeCode.files(&ctx, &Target::Wsl { distro: "U".into() }).unwrap();
        assert!(wsl.iter().any(|f| f.content.contains("/mnt/c/a/cli-companion.exe")));
        let version = |t: &Target| ClaudeCode.version(&ctx, t).unwrap();
        assert_ne!(version(&Target::Windows), version(&Target::Wsl { distro: "U".into() }));
        // Exe non traducibile per WSL: nessuna versione, nessun file.
        let odd = InstallCtx { exe: "relativo/cc.exe".to_string(), data_dir: "unused".into() };
        let wsl = Target::Wsl { distro: "U".into() };
        assert!(ClaudeCode.version(&odd, &wsl).is_none() && ClaudeCode.files(&odd, &wsl).is_err());
    }

    #[test]
    fn install_removes_obsolete_skill_folders() {
        let dir = std::env::temp_dir().join(format!("mc-obsolete-skills-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let skills = dir.join("skills");
        for old in ["orchestrate", "workflow", "task-intake"] {
            std::fs::create_dir_all(skills.join(old)).unwrap();
            std::fs::write(skills.join(old).join("SKILL.md"), "x").unwrap();
        }
        std::fs::write(skills.join("stray.txt"), "x").unwrap();
        remove_obsolete_skills(&dir).unwrap();
        assert!(!skills.join("orchestrate").exists() && !skills.join("workflow").exists());
        assert!(!skills.join("stray.txt").exists());
        // Le cartelle dei preset restano (verranno riscritte).
        assert!(skills.join("task-intake").join("SKILL.md").exists());
        // Senza cartella skills non c'e' nulla da fare.
        remove_obsolete_skills(&dir.join("missing")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plugin_listed_matches_exact_id_in_either_shape() {
        let id = PLUGIN_ID;
        let entry = format!(r#"{{"id":"{id}","version":"1.0.1","scope":"user","enabled":true}}"#);
        assert!(plugin_listed(&format!("[{entry}]"), id));
        assert!(plugin_listed(&format!(r#"{{"plugins":[{entry}]}}"#), id));
        // Rumore attorno al JSON.
        assert!(plugin_listed(&format!("avviso\n[{entry}]\n"), id));
        // Nessun match.
        assert!(!plugin_listed("[]", id));
        assert!(!plugin_listed("{}", id));
        assert!(!plugin_listed("non json", id));
        assert!(!plugin_listed("", id));
        assert!(!plugin_listed(r#"[{"id":"cli-companion-notify@altro"}]"#, id));
        assert!(!plugin_listed(r#"[{"id":"x-cli-companion-notify@cli-companion-local"}]"#, id));
        assert!(!plugin_listed(r#"[{"name":"cli-companion-notify@cli-companion-local"}]"#, id));
    }

    /// Con `claude` installato: il plugin generato deve passare la
    /// validazione reale. Non installa nulla nella config dell'utente.
    /// Esegui con `cargo test -- --ignored claude_validate`.
    #[test]
    #[ignore]
    fn claude_validate_accepts_generated_plugin() {
        let dir = std::env::temp_dir().join(format!("mc-claude-validate-{}", std::process::id()));
        let ctx =
            InstallCtx { exe: "C:/Program Files/cli-companion/cli-companion.exe".to_string(), data_dir: dir.clone() };
        let root = ctx.plugin_root(AGENT_ID, &Target::Windows);
        for PackageFile { path, content } in ClaudeCode.files(&ctx, &Target::Windows).unwrap() {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, content).unwrap();
        }
        let market = Target::Windows.path_arg(&root).unwrap();
        let plugin = format!("{market}/plugins/{PLUGIN}");
        let a = Target::Windows.run("claude", &["plugin", "validate", &plugin]);
        let b = Target::Windows.run("claude", &["plugin", "validate", &market]);
        let _ = std::fs::remove_dir_all(&dir);
        a.expect("validate plugin");
        b.expect("validate marketplace");
    }

    /// Rilevamento reale (richiede claude in Windows e/o nella distro).
    /// `cargo test -- --ignored claude_detect --nocapture`
    #[test]
    #[ignore]
    fn claude_detect_real_envs() {
        use crate::agents::AgentIntegration;
        let mut targets = vec![Target::Windows];
        targets.extend(
            crate::wsl::installed_distros().unwrap_or_default().into_iter().map(|distro| Target::Wsl { distro }),
        );
        for target in targets {
            let det = ClaudeCode.detect(&target);
            println!("{}: {:?} installato={}", target.key(), det, det.found && ClaudeCode.is_installed(&target, &det));
        }
    }

    /// Verifica reale dell'argv via WSL (`-e`): `plugin list --json` deve
    /// tornare JSON. `cargo test -- --ignored claude_wsl_argv --nocapture`
    #[test]
    #[ignore]
    fn claude_wsl_argv_survives_wsl_exe() {
        let distro = std::env::var("CC_TEST_DISTRO").unwrap_or_else(|_| "Ubuntu-24.04".to_string());
        let path = resolve_wsl_command(&distro, "claude").expect("claude nella distro");
        println!("claude: {path}");
        let out = Target::Wsl { distro }.run(&path, &["plugin", "list", "--json"]).expect("plugin list");
        println!("{out}");
        assert!(serde_json::from_str::<serde_json::Value>(out.trim()).is_ok(), "non e' JSON: {out}");
    }
}
