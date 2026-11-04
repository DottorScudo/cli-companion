//! Configurazione di un repository del workspace: preparazione e pulizia dei
//! worktree (vedi docs/BOARD.MD).

use super::e2e::E2eConfig;
use super::validate::{MAX_REPO_COMMANDS, MAX_REPO_COMMAND_LEN, MAX_REPO_PATH_LEN};
use crate::validate::{clip, is_absolute_like, path_segments, valid_branch, ErrorCode, ValidationError};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Preparazione e pulizia dei worktree di un repository (vedi docs/BOARD.MD).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct RepoConfig {
    /// Branch base dei task del repository (prima del default di `origin`).
    pub base_branch: Option<String>,
    /// Dove il repository e' montato in un container (es. `/var/www/html`):
    /// ne deriva `$CONTAINER_WORKTREE`.
    pub container_root: Option<String>,
    /// Percorsi (relativi al repository) ignorati da git da copiare nel
    /// worktree appena creato.
    pub copy_ignored: Vec<String>,
    /// Comandi eseguiti nel worktree appena creato, in ordine.
    pub setup: Vec<String>,
    /// Comandi eseguiti nel worktree prima di rimuoverlo.
    pub cleanup: Vec<String>,
    /// `git worktree remove --force`.
    pub remove_force: bool,
    /// Test E2E facoltativi del repository (vedi `E2eConfig`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub e2e: Option<E2eConfig>,
}

/// Quale elenco di comandi si sta validando: decide i codici di errore.
#[derive(Clone, Copy)]
enum CommandList {
    Setup,
    Cleanup,
}

impl CommandList {
    fn multiline(self) -> ErrorCode {
        match self {
            CommandList::Setup => ErrorCode::SetupCommandMultiline,
            CommandList::Cleanup => ErrorCode::CleanupCommandMultiline,
        }
    }

    fn too_long(self) -> ErrorCode {
        match self {
            CommandList::Setup => ErrorCode::SetupCommandTooLong,
            CommandList::Cleanup => ErrorCode::CleanupCommandTooLong,
        }
    }

    fn too_many(self) -> ErrorCode {
        match self {
            CommandList::Setup => ErrorCode::SetupCommandsTooMany,
            CommandList::Cleanup => ErrorCode::CleanupCommandsTooMany,
        }
    }
}

impl RepoConfig {
    /// Alla creazione del worktree c'e' qualcosa da fare?
    pub fn has_setup(&self) -> bool {
        !self.copy_ignored.is_empty() || !self.setup.is_empty()
    }

    /// Normalizza e valida: branch valido, percorsi relativi senza `..`,
    /// comandi non vuoti su una riga sola.
    pub fn normalized(mut self) -> Result<RepoConfig, ValidationError> {
        self.base_branch = normalize_base_branch(self.base_branch)?;
        self.container_root = normalize_container_root(self.container_root)?;
        self.copy_ignored = normalize_copy_paths(&self.copy_ignored)?;
        self.setup = normalize_commands(&self.setup, CommandList::Setup)?;
        self.cleanup = normalize_commands(&self.cleanup, CommandList::Cleanup)?;
        self.e2e = self.e2e.map(E2eConfig::normalized).transpose()?;
        Ok(self)
    }
}

fn trimmed_non_empty(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn normalize_base_branch(branch: Option<String>) -> Result<Option<String>, ValidationError> {
    match trimmed_non_empty(branch) {
        Some(b) if !valid_branch(&b) => Err(ValidationError::new(ErrorCode::BaseBranchInvalid).value(b)),
        other => Ok(other),
    }
}

fn normalize_container_root(root: Option<String>) -> Result<Option<String>, ValidationError> {
    match trimmed_non_empty(root) {
        Some(c) if c.chars().count() > MAX_REPO_PATH_LEN || c.chars().any(char::is_control) => {
            Err(ValidationError::new(ErrorCode::ContainerRootInvalid))
        }
        other => Ok(other.map(|c| c.trim_end_matches('/').to_string())),
    }
}

/// Percorsi da copiare: relativi al repository, `/` come separatore, senza
/// `..` ne' la radice, senza doppioni.
fn normalize_copy_paths(paths: &[String]) -> Result<Vec<String>, ValidationError> {
    let mut out: Vec<String> = Vec::new();
    for raw in paths {
        let t = raw.trim().replace('\\', "/");
        if t.is_empty() {
            continue;
        }
        let rejected = |code| ValidationError::new(code).value(t.clone());
        if is_absolute_like(&t) {
            return Err(rejected(ErrorCode::CopyPathNotRelative));
        }
        if t.chars().count() > MAX_REPO_PATH_LEN || t.chars().any(char::is_control) {
            return Err(ValidationError::new(ErrorCode::CopyPathInvalid).max(MAX_REPO_PATH_LEN).value(clip(&t)));
        }
        let segs = path_segments(&t);
        if segs.is_empty() || segs.contains(&"..") {
            return Err(rejected(ErrorCode::CopyPathEscapes));
        }
        let norm = segs.join("/");
        if !out.contains(&norm) {
            out.push(norm);
        }
    }
    if out.len() > MAX_REPO_COMMANDS {
        return Err(ValidationError::new(ErrorCode::CopyPathsTooMany).max(MAX_REPO_COMMANDS));
    }
    Ok(out)
}

/// Elenco di comandi: trim, righe vuote scartate; ciascuno su una riga, al
/// massimo `MAX_REPO_COMMAND_LEN` caratteri, al massimo `MAX_REPO_COMMANDS`.
fn normalize_commands(list: &[String], kind: CommandList) -> Result<Vec<String>, ValidationError> {
    let mut out = Vec::new();
    for raw in list {
        let c = raw.trim();
        if c.is_empty() {
            continue;
        }
        if c.chars().any(|ch| ch.is_control() && ch != '\t') {
            return Err(ValidationError::new(kind.multiline()).value(clip(c)));
        }
        if c.chars().count() > MAX_REPO_COMMAND_LEN {
            return Err(ValidationError::new(kind.too_long()).max(MAX_REPO_COMMAND_LEN));
        }
        out.push(c.to_string());
    }
    if out.len() > MAX_REPO_COMMANDS {
        return Err(ValidationError::new(kind.too_many()).max(MAX_REPO_COMMANDS));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspaces::model::Workspace;

    fn rc(f: impl FnOnce(&mut RepoConfig)) -> Result<RepoConfig, ValidationError> {
        let mut c = RepoConfig::default();
        f(&mut c);
        c.normalized()
    }

    fn code(r: Result<RepoConfig, ValidationError>) -> ErrorCode {
        r.unwrap_err().code()
    }

    #[test]
    fn is_normalized() {
        let c = rc(|c| {
            c.base_branch = Some("  develop ".into());
            c.container_root = Some(" /var/www/html/ ".into());
            c.copy_ignored = vec![
                " configuration.php ".into(),
                "".into(),
                ".\\public\\api\\.env".into(),
                "configuration.php".into(),
            ];
            c.setup = vec!["  composer dump-autoload ".into(), "   ".into()];
            c.cleanup = vec!["rm -rf x\ty".into()];
        })
        .unwrap();
        assert_eq!(c.base_branch.as_deref(), Some("develop"));
        assert_eq!(c.container_root.as_deref(), Some("/var/www/html"));
        assert_eq!(c.copy_ignored, ["configuration.php", "public/api/.env"]);
        assert_eq!(c.setup, ["composer dump-autoload"]);
        assert_eq!(c.cleanup, ["rm -rf x\ty"]);
        // Vuoti diventano None.
        let c = rc(|c| {
            c.base_branch = Some("  ".into());
            c.container_root = Some("".into());
        })
        .unwrap();
        assert!(c.base_branch.is_none() && c.container_root.is_none() && !c.has_setup());
    }

    #[test]
    fn rejects_bad_branches_and_container_roots() {
        for b in ["-x", "a b", "a..b", "x/", "/x"] {
            assert_eq!(code(rc(|c| c.base_branch = Some(b.into()))), ErrorCode::BaseBranchInvalid, "{b}");
        }
        assert_eq!(code(rc(|c| c.container_root = Some("/a\nb".into()))), ErrorCode::ContainerRootInvalid);
        assert_eq!(
            code(rc(|c| c.container_root = Some("a".repeat(MAX_REPO_PATH_LEN + 1)))),
            ErrorCode::ContainerRootInvalid
        );
    }

    #[test]
    fn rejects_bad_copy_paths() {
        for p in ["/etc/passwd", "C:\\x", "C:/x"] {
            assert_eq!(code(rc(|c| c.copy_ignored = vec![p.into()])), ErrorCode::CopyPathNotRelative, "{p}");
        }
        for p in ["../x", "a/../../b", "..", "."] {
            assert_eq!(code(rc(|c| c.copy_ignored = vec![p.into()])), ErrorCode::CopyPathEscapes, "{p}");
        }
        assert_eq!(code(rc(|c| c.copy_ignored = vec!["a\0b".into()])), ErrorCode::CopyPathInvalid);
        assert_eq!(code(rc(|c| c.copy_ignored = vec!["p".repeat(MAX_REPO_PATH_LEN + 1)])), ErrorCode::CopyPathInvalid);
        let many = (0..=MAX_REPO_COMMANDS).map(|i| format!("p{i}")).collect();
        assert_eq!(code(rc(|c| c.copy_ignored = many)), ErrorCode::CopyPathsTooMany);
    }

    #[test]
    fn rejects_bad_commands_per_list() {
        assert_eq!(code(rc(|c| c.setup = vec!["a\nb".into()])), ErrorCode::SetupCommandMultiline);
        assert_eq!(code(rc(|c| c.cleanup = vec!["a\rb".into()])), ErrorCode::CleanupCommandMultiline);
        let long = "x".repeat(MAX_REPO_COMMAND_LEN + 1);
        assert_eq!(code(rc(|c| c.setup = vec![long.clone()])), ErrorCode::SetupCommandTooLong);
        assert_eq!(code(rc(|c| c.cleanup = vec![long])), ErrorCode::CleanupCommandTooLong);
        assert!(rc(|c| c.setup = vec!["x".repeat(MAX_REPO_COMMAND_LEN)]).is_ok());
        assert_eq!(code(rc(|c| c.setup = vec!["x".into(); MAX_REPO_COMMANDS + 1])), ErrorCode::SetupCommandsTooMany);
        assert_eq!(
            code(rc(|c| c.cleanup = vec!["x".into(); MAX_REPO_COMMANDS + 1])),
            ErrorCode::CleanupCommandsTooMany
        );
        assert!(rc(|c| c.cleanup = vec!["x".into(); MAX_REPO_COMMANDS]).is_ok());
    }

    #[test]
    fn the_e2e_section_is_validated_and_optional() {
        let e2e =
            |command: &str, junit: &str| E2eConfig { command: command.into(), junit: junit.into(), timeout_min: 20 };
        let c = rc(|c| c.e2e = Some(e2e(" npm run e2e ", "out\\j.xml"))).unwrap();
        assert_eq!(c.e2e, Some(e2e("npm run e2e", "out/j.xml")));
        assert_eq!(code(rc(|c| c.e2e = Some(e2e("", "j.xml")))), ErrorCode::E2eCommandMissing);
        assert_eq!(code(rc(|c| c.e2e = Some(e2e("x", "../j.xml")))), ErrorCode::E2eJunitEscapes);
        // Una configurazione senza e2e, o con e2e senza timeout, si legge; senza e2e non lo scrive.
        let old: RepoConfig = serde_json::from_str(r#"{"setup":["s"]}"#).unwrap();
        assert!(old.e2e.is_none() && !serde_json::to_string(&old).unwrap().contains("e2e"));
        let new: RepoConfig = serde_json::from_str(r#"{"e2e":{"command":"c","junit":"j.xml"}}"#).unwrap();
        assert_eq!(new.e2e.as_ref().map(|e| e.timeout_min), Some(20));
        let back: RepoConfig = serde_json::from_str(&serde_json::to_string(&new).unwrap()).unwrap();
        assert_eq!(back, new);
    }

    #[test]
    fn a_workspace_without_repos_reads_and_writes_without_the_field() {
        let raw = r#"{"id":"ws-1","name":"p","kind":"windows","path":"C:\\p","created_at":1}"#;
        let mut ws: Workspace = serde_json::from_str(raw).unwrap();
        assert!(ws.repos.is_empty());
        assert!(!serde_json::to_string(&ws).unwrap().contains("repos"));
        ws.repos.insert("DDB".into(), RepoConfig { setup: vec!["x".into()], remove_force: true, ..Default::default() });
        let back: Workspace = serde_json::from_str(&serde_json::to_string(&ws).unwrap()).unwrap();
        assert_eq!(back.repos["DDB"].setup, ["x"]);
        assert!(back.repos["DDB"].remove_force);
    }

    #[test]
    fn messages_match_the_historic_texts() {
        let e = rc(|c| c.base_branch = Some("a b".into())).unwrap_err();
        assert_eq!(
            (e.message_it().as_str(), e.message_en().as_str()),
            ("branch base non valido: a b", "invalid base_branch: a b")
        );
        let e = rc(|c| c.copy_ignored = vec!["/abs".into()]).unwrap_err();
        assert_eq!(e.message_en(), "copy_ignored paths must be relative to the repository: /abs");
        let e = rc(|c| c.setup = vec!["a\nb".into()]).unwrap_err();
        assert_eq!(e.message_it(), "il comando di setup deve stare su una riga: a\nb");
    }
}
