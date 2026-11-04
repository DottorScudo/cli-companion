//! Contratto `IntegrationPackage` di OpenCode: la cartella del plugin (generata
//! da `plugin.rs`) copiata in `<config>/plugins/cli-companion-notify/`.
//!
//! Non si tocca nessun altro file dell'utente. La cartella di destinazione si
//! sovrascrive o si toglie SOLO se il suo `index.ts` porta la nostra
//! intestazione. Una copia del plugin e la destinazione scelta restano in
//! `plugin_root` (`destination.txt`), cosi' la rimozione cancella la stessa cartella.

use super::config_dir::config_dir;
use super::plugin::{is_ours, plugin_files, plugin_version_for};
use super::version::is_supported;
use super::{OpenCode, AGENT_ID, PLUGIN, SUPPORTED_MAJOR};
use crate::agents::{Detection, InstallCtx, IntegrationPackage, PackageFile};
use crate::platform::Target;
use std::path::{Path, PathBuf};

/// File di `plugin_root` con la cartella in cui il plugin e' stato installato.
const DESTINATION_RECORD: &str = "destination.txt";
/// Cartella dei plugin dentro la cartella di configurazione di OpenCode.
const PLUGINS_DIR: &str = "plugins";

/// Programma da lanciare, solo se l'agente c'e' ed e' di una versione supportata.
fn usable_program(det: &Detection) -> Result<&str, String> {
    let program =
        det.path.as_deref().filter(|_| det.found).ok_or_else(|| "OpenCode non trovato in questo env".to_string())?;
    if !is_supported(det.version.as_deref()) {
        return Err(format!(
            "OpenCode {} non e' supportato: serve la versione {SUPPORTED_MAJOR}.x",
            det.version.as_deref().unwrap_or("sconosciuta")
        ));
    }
    Ok(program)
}

/// Cartella del plugin dentro la configurazione `config`.
fn destination_in(config: &Path) -> PathBuf {
    config.join(PLUGINS_DIR).join(PLUGIN)
}

/// Scrive `files` (path relativi) sotto `dir`.
fn write_files(dir: &Path, files: &[PackageFile]) -> Result<(), String> {
    for PackageFile { path, content } in files {
        let file = dir.join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("scrittura plugin: {e}"))?;
        }
        std::fs::write(&file, content).map_err(|e| format!("scrittura plugin: {e}"))?;
    }
    Ok(())
}

/// Rifiuta una cartella di destinazione che esiste ma non e' nostra.
fn ensure_not_foreign(destination: &Path) -> Result<(), String> {
    if destination.exists() && !is_ours(destination) {
        return Err(format!(
            "La cartella {} esiste ma non e' stata creata da cli-companion: rinominala o eliminala e riprova.",
            destination.display()
        ));
    }
    Ok(())
}

/// Cartella registrata dall'ultima installazione, se c'e'.
fn recorded_destination(root: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(root.join(DESTINATION_RECORD)).ok()?;
    Some(PathBuf::from(text.trim())).filter(|p| !p.as_os_str().is_empty())
}

/// Dove sta (o starebbe) il plugin: la cartella registrata o quella derivata da `debug paths`.
fn known_destination(ctx: &InstallCtx, target: &Target, det: &Detection) -> Result<PathBuf, String> {
    if let Some(recorded) = recorded_destination(&ctx.plugin_root(AGENT_ID, target)) {
        return Ok(recorded);
    }
    let program = det.path.as_deref().filter(|_| det.found).ok_or_else(|| "OpenCode non trovato".to_string())?;
    config_dir(target, program).map(|config| destination_in(&config))
}

impl IntegrationPackage for OpenCode {
    fn version(&self, ctx: &InstallCtx, target: &Target) -> Option<String> {
        ctx.exe_for(target).ok().map(|exe| plugin_version_for(&exe))
    }

    fn files(&self, ctx: &InstallCtx, target: &Target) -> Result<Vec<PackageFile>, String> {
        let exe = ctx.exe_for(target)?;
        Ok(plugin_files(&exe).into_iter().map(|(path, content)| PackageFile { path, content }).collect())
    }

    fn is_installed(&self, target: &Target, det: &Detection) -> bool {
        let Ok(program) = usable_program(det) else {
            return false;
        };
        config_dir(target, program).is_ok_and(|config| is_ours(&destination_in(&config)))
    }

    fn install(&self, ctx: &InstallCtx, target: &Target, det: &Detection) -> Result<String, String> {
        let program = usable_program(det)?;
        let files = self.files(ctx, target)?;
        let destination = destination_in(&config_dir(target, program)?);
        ensure_not_foreign(&destination)?;

        // Copia in `plugin_root` (la sola cosa che il core conosce) e destinazione registrata.
        let root = ctx.plugin_root(AGENT_ID, target);
        let _ = std::fs::remove_dir_all(&root);
        write_files(&root, &files)?;
        std::fs::write(root.join(DESTINATION_RECORD), destination.to_string_lossy().as_bytes())
            .map_err(|e| format!("scrittura plugin: {e}"))?;

        // Niente avanzi: la cartella (nostra) si rifa da zero.
        if destination.exists() {
            std::fs::remove_dir_all(&destination).map_err(|e| format!("pulizia plugin precedente: {e}"))?;
        }
        write_files(&destination, &files)?;
        let log = [
            format!("plugin scritto in {}", root.display()),
            format!("plugin installato in {}", destination.display()),
            "stato: installato".to_string(),
        ];
        Ok(log.join("\n"))
    }

    fn uninstall(&self, ctx: &InstallCtx, target: &Target, det: &Detection) -> Result<String, String> {
        let mut log = Vec::new();
        match known_destination(ctx, target, det) {
            Ok(destination) if !destination.exists() => log.push("plugin gia' assente".to_string()),
            Ok(destination) if is_ours(&destination) => match std::fs::remove_dir_all(&destination) {
                Ok(()) => log.push(format!("plugin rimosso da {}", destination.display())),
                Err(e) => log.push(format!("rimozione plugin: {e}")),
            },
            Ok(destination) => {
                log.push(format!("{} non e' stata creata da cli-companion: lasciata dov'e'", destination.display()))
            }
            Err(e) => {
                log.push(format!("OpenCode: cartella del plugin non determinata ({e}): rimossi solo i file generati"))
            }
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
    use crate::agents::AgentIntegration;

    fn ctx(dir: &Path) -> InstallCtx {
        InstallCtx { exe: "C:/a/cli-companion.exe".to_string(), data_dir: dir.to_path_buf() }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mc-oc-package-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn det(version: Option<&str>) -> Detection {
        Detection { found: true, version: version.map(str::to_string), path: Some("opencode".to_string()) }
    }

    #[test]
    fn files_are_the_generated_plugin_for_the_target_exe() {
        let ctx = ctx(Path::new("unused"));
        let win = OpenCode.files(&ctx, &Target::Windows).unwrap();
        assert!(win.iter().any(|f| f.path == "index.ts" && f.content.contains("C:/a/cli-companion.exe")));
        assert!(win.iter().any(|f| f.path == "skills/task-intake/SKILL.md"));
        let wsl = OpenCode.files(&ctx, &Target::Wsl { distro: "U".into() }).unwrap();
        assert!(wsl.iter().any(|f| f.path == "index.ts" && f.content.contains("/mnt/c/a/cli-companion.exe")));
        let version = |t: &Target| OpenCode.version(&ctx, t).unwrap();
        assert_ne!(version(&Target::Windows), version(&Target::Wsl { distro: "U".into() }));
        // Exe non traducibile per WSL: nessuna versione, nessun file.
        let odd = InstallCtx { exe: "relativo/cc.exe".to_string(), data_dir: "unused".into() };
        let wsl = Target::Wsl { distro: "U".into() };
        assert!(OpenCode.version(&odd, &wsl).is_none() && OpenCode.files(&odd, &wsl).is_err());
    }

    #[test]
    fn only_a_found_and_supported_version_is_usable() {
        assert_eq!(usable_program(&det(Some("2.0.30"))), Ok("opencode"));
        let err = usable_program(&det(Some("1.14.3"))).unwrap_err();
        assert!(err.contains("1.14.3") && err.contains("2.x"), "{err}");
        assert!(usable_program(&det(None)).unwrap_err().contains("sconosciuta"));
        assert!(usable_program(&Detection::default()).is_err());
        let not_found = Detection { found: false, ..det(Some("2.0.30")) };
        assert!(usable_program(&not_found).is_err());
    }

    #[test]
    fn an_unsupported_version_is_neither_installed_nor_installable() {
        let dir = temp_dir("unsupported");
        let old = det(Some("1.14.3"));
        assert!(!OpenCode.is_installed(&Target::Windows, &old));
        assert!(OpenCode.install(&ctx(&dir), &Target::Windows, &old).unwrap_err().contains("2.x"));
        assert!(!dir.exists(), "nulla scritto");
    }

    #[test]
    fn the_destination_is_the_plugin_folder_inside_the_config() {
        assert_eq!(destination_in(Path::new("cfg")), Path::new("cfg").join("plugins").join("cli-companion-notify"));
    }

    #[test]
    fn a_foreign_folder_is_refused_and_ours_is_accepted() {
        let dir = temp_dir("foreign");
        let dest = dir.join("dest");
        assert!(ensure_not_foreign(&dest).is_ok(), "assente: va bene");
        std::fs::create_dir_all(&dest).unwrap();
        assert!(ensure_not_foreign(&dest).unwrap_err().contains("non e' stata creata da cli-companion"));
        std::fs::write(dest.join("index.ts"), "export default {}").unwrap();
        assert!(ensure_not_foreign(&dest).is_err());
        let files = OpenCode.files(&ctx(&dir), &Target::Windows).unwrap();
        write_files(&dest, &files).unwrap();
        assert!(ensure_not_foreign(&dest).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writing_replaces_the_folder_without_leftovers() {
        let dir = temp_dir("leftovers");
        let dest = dir.join("dest");
        let files = OpenCode.files(&ctx(&dir), &Target::Windows).unwrap();
        write_files(&dest, &files).unwrap();
        for stale in ["skills/orchestrate/SKILL.md", "old.ts"] {
            std::fs::create_dir_all(dest.join(stale).parent().unwrap()).unwrap();
            std::fs::write(dest.join(stale), "x").unwrap();
        }
        // Come in `install`: cartella nostra, si rifa da zero.
        assert!(is_ours(&dest));
        std::fs::remove_dir_all(&dest).unwrap();
        write_files(&dest, &files).unwrap();
        assert!(!dest.join("old.ts").exists() && !dest.join("skills/orchestrate").exists());
        assert!(dest.join("skills/task-intake/SKILL.md").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_recorded_destination_is_read_back() {
        let dir = temp_dir("record");
        assert_eq!(recorded_destination(&dir), None);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(DESTINATION_RECORD), "  \n").unwrap();
        assert_eq!(recorded_destination(&dir), None);
        std::fs::write(
            dir.join(DESTINATION_RECORD),
            "C:\\Users\\me\\.config\\opencode\\plugins\\cli-companion-notify\n",
        )
        .unwrap();
        assert_eq!(
            recorded_destination(&dir),
            Some(PathBuf::from("C:\\Users\\me\\.config\\opencode\\plugins\\cli-companion-notify"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn uninstall_removes_our_folder_by_the_record_and_the_generated_files() {
        let dir = temp_dir("uninstall");
        let ctx = ctx(&dir);
        let target = Target::Windows;
        let dest = dir.join("config").join("plugins").join(PLUGIN);
        write_files(&dest, &OpenCode.files(&ctx, &target).unwrap()).unwrap();
        let root = ctx.plugin_root(AGENT_ID, &target);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(DESTINATION_RECORD), dest.to_string_lossy().as_bytes()).unwrap();
        let log = OpenCode.uninstall(&ctx, &target, &Detection::default()).unwrap();
        assert!(!dest.exists() && !root.exists(), "{log}");
        assert!(log.contains("plugin rimosso") && log.contains("file generati eliminati"), "{log}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn uninstall_never_removes_a_folder_that_is_not_ours() {
        let dir = temp_dir("uninstall-foreign");
        let ctx = ctx(&dir);
        let target = Target::Windows;
        let dest = dir.join("plugins").join(PLUGIN);
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("index.ts"), "// mine").unwrap();
        let root = ctx.plugin_root(AGENT_ID, &target);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(DESTINATION_RECORD), dest.to_string_lossy().as_bytes()).unwrap();
        let log = OpenCode.uninstall(&ctx, &target, &det(Some("2.0.30"))).unwrap();
        assert!(dest.join("index.ts").exists(), "{log}");
        assert!(log.contains("lasciata dov'e'"), "{log}");
        assert!(!root.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn uninstall_without_anything_is_harmless() {
        let dir = temp_dir("uninstall-empty");
        let log = OpenCode.uninstall(&ctx(&dir), &Target::Windows, &Detection::default()).unwrap();
        assert!(log.contains("rimossi solo i file generati"), "{log}");
    }

    /// Con OpenCode 2.x installato: legge davvero `debug paths` e genera il plugin
    /// in una cartella temporanea (non installa nulla nella config dell'utente).
    /// `cargo test -- --ignored opencode_real --nocapture`
    #[test]
    #[ignore]
    fn opencode_real_detect_and_generate() {
        let dir = temp_dir("real");
        let det = OpenCode.detect(&Target::Windows);
        println!("rilevamento: {det:?} installato={}", det.found && OpenCode.is_installed(&Target::Windows, &det));
        let program = usable_program(&det).expect("OpenCode 2.x installato");
        let config = config_dir(&Target::Windows, program).expect("debug paths");
        println!("config: {}", config.display());
        assert!(config.is_absolute());
        let ctx =
            InstallCtx { exe: "C:/Program Files/cli-companion/cli-companion.exe".to_string(), data_dir: dir.clone() };
        let files = OpenCode.files(&ctx, &Target::Windows).unwrap();
        let temp_destination = dir.join("plugins").join(PLUGIN);
        write_files(&temp_destination, &files).unwrap();
        assert!(is_ours(&temp_destination));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ciclo reale installa/stato/rimuovi in una home finta: OpenCode legge
    /// `USERPROFILE`, quindi la configurazione dell'utente non si tocca (il test
    /// si ferma se `debug paths` non punta dentro la home finta). Lascia il
    /// plugin installato se `CC_OPENCODE_TEST_KEEP` e' impostata, per provarlo.
    /// `CC_OPENCODE_TEST_HOME=<dir> USERPROFILE=<dir> cargo test -- --ignored opencode_real_install_cycle --nocapture`
    #[test]
    #[ignore]
    fn opencode_real_install_cycle_in_a_fake_home() {
        let home = PathBuf::from(std::env::var("CC_OPENCODE_TEST_HOME").expect("CC_OPENCODE_TEST_HOME"));
        let det = OpenCode.detect(&Target::Windows);
        let config = config_dir(&Target::Windows, usable_program(&det).unwrap()).unwrap();
        assert!(config.starts_with(&home), "debug paths fuori dalla home finta: {}", config.display());
        let exe = std::env::var("CC_OPENCODE_TEST_EXE").unwrap_or_else(|_| "C:/a/cli-companion.exe".to_string());
        let ctx = InstallCtx { exe, data_dir: home.join("app-data") };
        println!("{}", OpenCode.install(&ctx, &Target::Windows, &det).unwrap());
        assert!(OpenCode.is_installed(&Target::Windows, &det));
        if std::env::var("CC_OPENCODE_TEST_KEEP").is_ok() {
            return;
        }
        println!("{}", OpenCode.uninstall(&ctx, &Target::Windows, &det).unwrap());
        assert!(!OpenCode.is_installed(&Target::Windows, &det));
        assert!(!destination_in(&config).exists());
    }
}
