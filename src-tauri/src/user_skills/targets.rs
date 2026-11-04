//! Dove le skill utente vengono sincronizzate: la cartella delle skill di ogni
//! integrazione che dichiara il contratto `UserSkills`, su Windows e nella
//! home Linux di ogni distro WSL "in uso", cioe' dove e' installato il
//! pacchetto di quell'integrazione. Un'integrazione senza contratto non ha
//! destinazioni.
//!
//! Scelta di design: la scrittura nelle distro passa dal path UNC
//! (`\\wsl.localhost\<distro>\home\<u>\<cartella skill>`) visto da Windows,
//! cosi' tutta la logica di `user_skills` (marcatore `.cli-companion-managed`,
//! cartelle non gestite mai toccate, symlink non seguiti, staging + rename,
//! slug validati contro il path traversal) vale identica, senza costruire
//! alcun comando di shell con contenuto dell'utente.
//!
//! Accedere a un UNC di una distro ferma la avvia: e' accettabile solo perche'
//! i target sono limitati alle distro in uso. La home si risolve prima (con
//! timeout, in parallelo fra le distro): una distro irraggiungibile o appesa
//! diventa `dir: Err(..)` (`ok: false` nei report) senza bloccare le altre.
//! Le operazioni su file via UNC non hanno un timeout proprio, ma partono solo
//! dopo che la distro ha risposto alla risoluzione della home.

use crate::agents::{plugins_dir, registry, sanitize_env_key, AgentIntegration, UserSkills};
use crate::platform::{is_valid_distro_name, wsl_to_windows_path, WINDOWS_KEY};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

/// Nome dell'ambiente Windows nei risultati dei tool delle skill.
pub const WINDOWS_ENV: &str = WINDOWS_KEY;

/// Prefisso delle distro interne di Docker, senza una shell utilizzabile.
const DOCKER_DISTRO_PREFIX: &str = "docker-desktop";

/// Ambiente in cui le skill devono comparire e la sua cartella delle skill
/// (`Err` se l'ambiente non e' raggiungibile). Windows piu' le distro WSL in
/// uso, per ogni integrazione che dichiara `UserSkills`: `env` nomina
/// l'ambiente, non l'agente.
pub struct SkillTarget {
    pub env: String,
    pub dir: Result<PathBuf, String>,
}

/// Integrazioni che dichiarano `UserSkills`, col loro contratto.
fn with_skills<'a>(integrations: &[&'a dyn AgentIntegration]) -> Vec<(&'a dyn AgentIntegration, &'a dyn UserSkills)> {
    integrations.iter().filter_map(|i| i.capabilities().user_skills.map(|skills| (*i, skills))).collect()
}

/// I soli target Windows (sempre per primi), uno per integrazione con il contratto.
pub fn windows_only() -> Vec<SkillTarget> {
    windows_targets(&with_skills(registry::all()))
}

fn windows_targets(integrations: &[(&dyn AgentIntegration, &dyn UserSkills)]) -> Vec<SkillTarget> {
    integrations
        .iter()
        .map(|(_, skills)| SkillTarget { env: WINDOWS_ENV.to_string(), dir: skills.windows_dir() })
        .collect()
}

/// Tutti i target: Windows per primo, poi le distro in uso.
pub fn all(data_dir: &Path) -> Vec<SkillTarget> {
    targets_with(data_dir, registry::all(), crate::wsl::installed_distros, crate::wsl::linux_home)
}

/// Distro installate (nell'ordine di `installed`) in cui c'e' il pacchetto
/// dell'integrazione: nome valido per un UNC, non interna di Docker e con la
/// cartella del pacchetto fra `plugin_env_dirs`.
pub fn distros_in_use(plugin_env_dirs: &[String], installed: &[String]) -> Vec<String> {
    installed
        .iter()
        .filter(|d| !d.to_lowercase().starts_with(DOCKER_DISTRO_PREFIX))
        .filter(|d| is_valid_distro_name(d))
        .filter(|d| plugin_env_dirs.contains(&sanitize_env_key(d)))
        .cloned()
        .collect()
}

/// Cartella skill UNC di una distro data la sua home Linux e il sottopercorso
/// dichiarato dall'integrazione.
pub fn wsl_skills_dir(distro: &str, home: &str, skills_subpath: &str) -> Result<PathBuf, String> {
    let linux = format!("{}/{skills_subpath}", home.trim_end_matches('/'));
    wsl_to_windows_path(distro, &linux)
        .map(PathBuf::from)
        .ok_or_else(|| format!("Cannot build the skills path of WSL distro '{distro}'."))
}

/// Nomi delle cartelle ambiente del pacchetto di `agent_id`, escluso Windows.
/// Cartella assente = nessun ambiente.
fn plugin_env_dirs(data_dir: &Path, agent_id: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir(plugins_dir(data_dir, agent_id)) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| name != WINDOWS_ENV)
        .collect()
}

/// Nucleo di `all` con gli effetti iniettati. `installed` (lento) viene
/// chiamato solo se c'e' almeno una cartella pacchetto non Windows; le home
/// delle distro in uso si risolvono in parallelo, l'ordine dei target e'
/// preservato.
fn targets_with(
    data_dir: &Path,
    integrations: &[&dyn AgentIntegration],
    installed: impl FnOnce() -> Result<Vec<String>, String>,
    home: impl Fn(&str) -> Result<String, String> + Sync,
) -> Vec<SkillTarget> {
    let integrations = with_skills(integrations);
    let mut targets = windows_targets(&integrations);
    let env_dirs: Vec<_> = integrations
        .iter()
        .map(|(integration, skills)| (*skills, plugin_env_dirs(data_dir, integration.id())))
        .filter(|(_, dirs)| !dirs.is_empty())
        .collect();
    if env_dirs.is_empty() {
        return targets;
    }
    let installed = match installed() {
        Ok(installed) => installed,
        Err(e) => {
            eprintln!("user skills: cannot list the WSL distros: {e}");
            return targets;
        }
    };
    let wanted: Vec<(&dyn UserSkills, String)> = env_dirs
        .iter()
        .flat_map(|(skills, dirs)| distros_in_use(dirs, &installed).into_iter().map(move |distro| (*skills, distro)))
        .collect();
    let home = &home;
    let dirs: Vec<Result<PathBuf, String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = wanted
            .iter()
            .map(|(skills, distro)| {
                let subpath = skills.wsl_home_subpath();
                scope.spawn(move || home(distro).and_then(|h| wsl_skills_dir(distro, &h, subpath)))
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("Home resolution panicked.".to_string()))).collect()
    });
    targets.extend(wanted.into_iter().zip(dirs).map(|((_, env), dir)| SkillTarget { env, dir }));
    targets
}
