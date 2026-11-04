//! Tabella unica dei metodi RPC e dei ruoli che possono chiamarli. `dispatch`
//! la usa per il controllo di accesso e lo smistamento, il server MCP per
//! scegliere i tool di ogni ruolo (un test verifica che coincidano): aggiungere
//! un metodo obbliga a decidere chi lo puo' chiamare, perche' `Method::roles`
//! e' un `match` esaustivo.

use crate::board::TaskKind;

/// Chi sta chiamando, come lo riporta `whoami`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Qualunque tab di cli-companion che non e' il worker di un task (Intake compreso).
    Session,
    /// Il worker di un task `work`.
    Worker,
    /// Il worker di un task `review`: verifica il risultato integrato e propone le correzioni.
    Reviewer,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Session, Role::Worker, Role::Reviewer];

    /// Valore di `whoami` sul filo.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Session => "session",
            Role::Worker => "worker",
            Role::Reviewer => "reviewer",
        }
    }

    pub fn parse(s: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.as_str() == s)
    }

    /// Ruolo di un tab dal tipo del task che sta eseguendo (`None` = nessun task).
    pub fn of_worker(kind: Option<TaskKind>) -> Role {
        match kind {
            None => Role::Session,
            Some(TaskKind::Work) => Role::Worker,
            Some(TaskKind::Review) => Role::Reviewer,
        }
    }
}

/// Metodo RPC, con il nome che usa sul filo (e come tool MCP).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Handshake del server MCP: non e' un tool.
    Whoami,
    GetTask,
    CompleteTask,
    RunE2e,
    GetE2eResult,
    ProposeTasks,
    ListTasks,
    GetTaskDetails,
    UpdateTask,
    DeleteTask,
    SaveSkill,
    ListSkills,
    DeleteSkill,
    GetWorkspaceSettings,
    ProposeSettings,
}

/// Tutti i ruoli.
const EVERYONE: &[Role] = &Role::ALL;
/// I worker automatici: consegnano un task e ne riferiscono l'esito.
const AUTOMATED: &[Role] = &[Role::Worker, Role::Reviewer];
/// Gestione della coda di task: il revisore ne ha solo una parte.
const SESSION_AND_REVIEWER: &[Role] = &[Role::Session, Role::Reviewer];
/// Strumenti dell'utente (coda, skill, impostazioni): vietati ai worker.
const SESSION_ONLY: &[Role] = &[Role::Session];

impl Method {
    pub const ALL: [Method; 15] = [
        Method::Whoami,
        Method::GetTask,
        Method::CompleteTask,
        Method::RunE2e,
        Method::GetE2eResult,
        Method::ProposeTasks,
        Method::ListTasks,
        Method::GetTaskDetails,
        Method::UpdateTask,
        Method::DeleteTask,
        Method::SaveSkill,
        Method::ListSkills,
        Method::DeleteSkill,
        Method::GetWorkspaceSettings,
        Method::ProposeSettings,
    ];

    /// Nome sul filo.
    pub fn name(self) -> &'static str {
        match self {
            Method::Whoami => "whoami",
            Method::GetTask => "get_task",
            Method::CompleteTask => "complete_task",
            Method::RunE2e => "run_e2e",
            Method::GetE2eResult => "get_e2e_result",
            Method::ProposeTasks => "propose_tasks",
            Method::ListTasks => "list_tasks",
            Method::GetTaskDetails => "get_task_details",
            Method::UpdateTask => "update_task",
            Method::DeleteTask => "delete_task",
            Method::SaveSkill => "save_skill",
            Method::ListSkills => "list_skills",
            Method::DeleteSkill => "delete_skill",
            Method::GetWorkspaceSettings => "get_workspace_settings",
            Method::ProposeSettings => "propose_settings",
        }
    }

    pub fn parse(name: &str) -> Option<Method> {
        Method::ALL.into_iter().find(|m| m.name() == name)
    }

    /// I ruoli che possono chiamare il metodo.
    pub fn roles(self) -> &'static [Role] {
        match self {
            Method::Whoami => EVERYONE,
            Method::GetTask | Method::CompleteTask | Method::RunE2e | Method::GetE2eResult => AUTOMATED,
            Method::ProposeTasks | Method::GetTaskDetails => SESSION_AND_REVIEWER,
            Method::ListTasks
            | Method::UpdateTask
            | Method::DeleteTask
            | Method::SaveSkill
            | Method::ListSkills
            | Method::DeleteSkill
            | Method::GetWorkspaceSettings
            | Method::ProposeSettings => SESSION_ONLY,
        }
    }

    pub fn allows(self, role: Role) -> bool {
        self.roles().contains(&role)
    }

    /// `true` per l'handshake, che non ha un tool MCP.
    pub fn is_handshake(self) -> bool {
        match self {
            Method::Whoami => true,
            Method::GetTask
            | Method::CompleteTask
            | Method::RunE2e
            | Method::GetE2eResult
            | Method::ProposeTasks
            | Method::ListTasks
            | Method::GetTaskDetails
            | Method::UpdateTask
            | Method::DeleteTask
            | Method::SaveSkill
            | Method::ListSkills
            | Method::DeleteSkill
            | Method::GetWorkspaceSettings
            | Method::ProposeSettings => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_method_name_parses_back_to_the_method() {
        for m in Method::ALL {
            assert_eq!(Method::parse(m.name()), Some(m), "{}", m.name());
        }
        assert_eq!(Method::parse("get_events"), None);
        assert_eq!(Method::parse(""), None);
        assert_eq!(Method::parse("GET_TASK"), None);
    }

    #[test]
    fn method_names_are_unique() {
        let names: HashSet<&str> = Method::ALL.iter().map(|m| m.name()).collect();
        assert_eq!(names.len(), Method::ALL.len());
    }

    #[test]
    fn every_role_string_parses_back_to_the_role() {
        for r in Role::ALL {
            assert_eq!(Role::parse(r.as_str()), Some(r), "{}", r.as_str());
        }
        let names: HashSet<&str> = Role::ALL.iter().map(|r| r.as_str()).collect();
        assert_eq!(names.len(), Role::ALL.len());
        assert_eq!(Role::parse("admin"), None);
        assert_eq!(Role::parse(""), None);
    }

    #[test]
    fn the_role_comes_from_the_kind_of_the_task() {
        assert_eq!(Role::of_worker(None), Role::Session);
        assert_eq!(Role::of_worker(Some(TaskKind::Work)), Role::Worker);
        assert_eq!(Role::of_worker(Some(TaskKind::Review)), Role::Reviewer);
    }

    #[test]
    fn whoami_is_the_only_handshake_and_every_role_may_call_it() {
        for r in Role::ALL {
            assert!(Method::Whoami.allows(r), "{}", r.as_str());
        }
        let handshakes: Vec<Method> = Method::ALL.into_iter().filter(|m| m.is_handshake()).collect();
        assert_eq!(handshakes, vec![Method::Whoami]);
    }

    #[test]
    fn allows_agrees_with_the_role_list_and_no_method_is_orphaned() {
        for m in Method::ALL {
            assert!(!m.roles().is_empty(), "{} nobody can call it", m.name());
            for r in Role::ALL {
                assert_eq!(m.allows(r), m.roles().contains(&r), "{} / {}", m.name(), r.as_str());
            }
        }
    }

    #[test]
    fn the_table_keeps_the_agreed_permissions() {
        let who = |m: Method| Role::ALL.into_iter().filter(|r| m.allows(*r)).collect::<Vec<_>>();
        let automated = vec![Role::Worker, Role::Reviewer];
        let shared = vec![Role::Session, Role::Reviewer];
        for m in [Method::GetTask, Method::CompleteTask, Method::RunE2e, Method::GetE2eResult] {
            assert_eq!(who(m), automated, "{}", m.name());
        }
        for m in [Method::ProposeTasks, Method::GetTaskDetails] {
            assert_eq!(who(m), shared, "{}", m.name());
        }
        for m in [
            Method::ListTasks,
            Method::UpdateTask,
            Method::DeleteTask,
            Method::SaveSkill,
            Method::ListSkills,
            Method::DeleteSkill,
            Method::GetWorkspaceSettings,
            Method::ProposeSettings,
        ] {
            assert_eq!(who(m), vec![Role::Session], "{}", m.name());
        }
    }
}
