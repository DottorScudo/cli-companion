//! Operazioni sulla coda: task nuovi e proposte, modifiche, cancellazioni e
//! riordino. Ogni operazione verifica lo stato del task e non tocca il suo
//! ciclo di vita (vedi `lifecycle`).

use super::{
    check_review_deps, parse_spec, permits, valid_id, validate_graph, Board, LifecycleError, Op, Proposal, Task,
    TaskAction, TaskSpec, TaskStatus, MAX_PENDING_PROPOSALS, MAX_TASKS_PER_BOARD,
};
use crate::validate::clip_chars;
use serde_json::Value;
use std::collections::HashSet;

impl Board {
    // --- task nuovi e proposte ---

    /// Task ancora da concludere (`queued` o `running`): quelli che contano nel limite.
    fn unfinished_count(&self) -> usize {
        self.tasks.iter().filter(|t| matches!(t.status(), TaskStatus::Queued | TaskStatus::Running)).count()
    }

    /// Verifica i task nuovi contro la board: id validi e unici (anche rispetto
    /// a `reserved`, gli id delle altre proposte pendenti), dipendenze verso la
    /// proposta o task esistenti, niente cicli, limiti.
    pub fn check_specs(&self, specs: &[TaskSpec], reserved: &HashSet<&str>) -> Result<(), String> {
        if specs.is_empty() {
            return Err("tasks is empty".to_string());
        }
        if self.unfinished_count() + specs.len() > MAX_TASKS_PER_BOARD {
            return Err(format!(
                "too many unfinished tasks on the board (max {MAX_TASKS_PER_BOARD} queued or running)"
            ));
        }
        let mut seen: HashSet<&str> = self.tasks.iter().map(|t| t.id.as_str()).collect();
        for s in specs {
            if !valid_id(&s.id) {
                return Err(format!("invalid task id: {}", clip_chars(&s.id, 50)));
            }
            check_review_deps(s)?;
            if reserved.contains(s.id.as_str()) {
                return Err(format!("duplicate task id: {} (already used by a pending proposal)", s.id));
            }
            if !seen.insert(s.id.as_str()) {
                return Err(format!("duplicate task id: {}", s.id));
            }
        }
        let mut all = self.tasks.clone();
        all.extend(specs.iter().cloned().map(|s| Task::from_spec(s, 0)));
        validate_graph(&all)
    }

    fn pending_ids(&self, except: Option<&str>) -> HashSet<&str> {
        self.proposals
            .iter()
            .filter(|p| Some(p.id.as_str()) != except)
            .flat_map(|p| p.tasks.iter().map(|t| t.id.as_str()))
            .collect()
    }

    /// Id non ancora usati da task o proposte (per i task creati a mano).
    pub fn id_taken(&self, id: &str) -> bool {
        self.task(id).is_some() || self.pending_ids(None).contains(id)
    }

    /// Aggiunge in fondo alla coda task gia' verificati con `check_specs`.
    pub(super) fn push_specs(&mut self, specs: Vec<TaskSpec>, now: i64) -> Vec<String> {
        let ids = specs.iter().map(|s| s.id.clone()).collect();
        self.tasks.extend(specs.into_iter().map(|s| Task::from_spec(s, now)));
        self.updated_at = now;
        ids
    }

    /// Task creato a mano, senza proposta.
    pub fn create_task(&mut self, spec: TaskSpec, now: i64) -> Result<String, String> {
        self.check_specs(std::slice::from_ref(&spec), &self.pending_ids(None))?;
        Ok(self.push_specs(vec![spec], now).remove(0))
    }

    /// Nuova proposta pendente (dell'Intake o di un revisore).
    pub fn add_proposal(
        &mut self,
        id: String,
        source_tab: &str,
        source_task: Option<String>,
        specs: Vec<TaskSpec>,
        now: i64,
    ) -> Result<(), String> {
        if self.proposals.len() >= MAX_PENDING_PROPOSALS {
            return Err(format!(
                "too many pending proposals (max {MAX_PENDING_PROPOSALS}): wait for the user to accept or discard some"
            ));
        }
        self.check_specs(&specs, &self.pending_ids(None))?;
        self.proposals.push(Proposal {
            id,
            source_tab: source_tab.to_string(),
            source_task,
            created_at: now,
            tasks: specs,
        });
        self.updated_at = now;
        Ok(())
    }

    pub fn proposal(&self, id: &str) -> Result<&Proposal, String> {
        self.proposals.iter().find(|p| p.id == id).ok_or_else(|| format!("proposal not found: {id}"))
    }

    /// Accetta una proposta: dopo una nuova validazione i task entrano in
    /// coda, nell'ordine della proposta. Se la validazione fallisce la
    /// proposta resta pendente.
    pub fn accept_proposal(&mut self, id: &str, now: i64) -> Result<Vec<String>, String> {
        let specs = self.proposal(id)?.tasks.clone();
        self.check_specs(&specs, &HashSet::new())?;
        self.proposals.retain(|p| p.id != id);
        Ok(self.push_specs(specs, now))
    }

    pub fn discard_proposal(&mut self, id: &str, now: i64) -> Result<(), String> {
        self.proposal(id)?;
        self.proposals.retain(|p| p.id != id);
        self.updated_at = now;
        Ok(())
    }

    // --- modifiche dei task ---

    /// Modifica parziale di un task `queued` o `failed` (`patch` = campi di
    /// `TaskSpec`); `repo` solo senza worktree.
    pub fn update_task(&mut self, id: &str, patch: &Value, now: i64) -> Result<(), String> {
        let t = self.require(id, Op::Edit)?;
        let spec = parse_spec(patch, Some(&TaskSpec::from_task(t)))?;
        check_review_deps(&spec)?;
        if patch.as_object().is_some_and(|o| o.keys().all(|k| k == "id")) {
            return Err("no fields to update".to_string());
        }
        if spec.repo != t.repo && t.worktree.is_some() {
            return Err(format!("task '{id}' already has a worktree: its repo cannot be changed"));
        }
        let mut all = self.tasks.clone();
        if let Some(slot) = all.iter_mut().find(|x| x.id == id) {
            slot.apply_spec(spec, now);
        }
        validate_graph(&all)?;
        self.tasks = all;
        self.updated_at = now;
        Ok(())
    }

    /// Toglie un task `queued` o `failed` che nessun task non `done` aspetta.
    /// Ritorna il task rimosso (il suo worktree va ripulito da chi chiama).
    pub fn delete_task(&mut self, id: &str, now: i64) -> Result<Task, String> {
        self.require(id, Op::Action(TaskAction::Delete))?;
        let waiting: Vec<&str> = self
            .tasks
            .iter()
            .filter(|x| x.status() != TaskStatus::Done && x.depends_on.iter().any(|d| d == id))
            .map(|x| x.id.as_str())
            .collect();
        if !waiting.is_empty() {
            return Err(format!(
                "task '{id}' cannot be deleted: {} depend on it. Remove it from their depends_on (update_task) or delete them first",
                waiting.join(", ")
            ));
        }
        let removed = self.take_task(id)?;
        self.drop_references(&[removed.id.as_str()]);
        self.updated_at = now;
        Ok(removed)
    }

    /// Toglie un task `queued` o `failed` insieme a tutti i task non `done` che
    /// ne dipendono, direttamente o no. Tutto o niente: se uno dei task da
    /// togliere e' `running` non cambia nulla. Ritorna i task rimossi (i loro
    /// worktree vanno ripuliti da chi chiama).
    pub fn delete_with_dependents(&mut self, id: &str, now: i64) -> Result<Vec<Task>, String> {
        let t = self.require(id, Op::Action(TaskAction::DeleteCascade))?;
        let cascade = self.waiters().cascade_of(t);
        if !cascade.running.is_empty() {
            return Err(format!("cannot delete: {} are running, stop them first", cascade.running.join(", ")));
        }
        let doomed: HashSet<&str> = std::iter::once(id).chain(cascade.dependents.iter().map(String::as_str)).collect();
        let (removed, kept): (Vec<Task>, Vec<Task>) =
            std::mem::take(&mut self.tasks).into_iter().partition(|x| doomed.contains(x.id.as_str()));
        self.tasks = kept;
        let removed_ids: Vec<&str> = removed.iter().map(|x| x.id.as_str()).collect();
        self.drop_references(&removed_ids);
        self.updated_at = now;
        Ok(removed)
    }

    /// Toglie il task `id` dalla coda.
    fn take_task(&mut self, id: &str) -> Result<Task, LifecycleError> {
        let pos = self.tasks.iter().position(|x| x.id == id).ok_or_else(|| LifecycleError::NotFound(id.to_string()))?;
        Ok(self.tasks.remove(pos))
    }

    /// I task rimasti (anche `done`) non devono tenere un riferimento a un task sparito.
    fn drop_references(&mut self, removed: &[&str]) {
        for x in &mut self.tasks {
            x.depends_on.retain(|d| !removed.contains(&d.as_str()));
        }
    }

    /// Sposta un `queued` prima di `before` (un altro `queued`) o in fondo.
    pub fn move_task(&mut self, id: &str, before: Option<&str>, now: i64) -> Result<(), String> {
        self.require(id, Op::Reorder)?;
        if let Some(b) = before {
            if b != id && !permits(self.known(b)?.status(), Op::Reorder) {
                return Err(format!("task '{b}' is not queued: a task can only be placed before a queued task"));
            }
        }
        if before == Some(id) {
            return Ok(());
        }
        let t = self.take_task(id)?;
        let at = before.and_then(|b| self.tasks.iter().position(|x| x.id == b)).unwrap_or(self.tasks.len());
        self.tasks.insert(at, t);
        self.updated_at = now;
        Ok(())
    }

    /// Porta un `queued` in testa alla coda.
    pub fn task_to_top(&mut self, id: &str, now: i64) -> Result<(), String> {
        self.require(id, Op::Action(TaskAction::ToTop))?;
        let t = self.take_task(id)?;
        let at = self.tasks.iter().position(|x| x.status() == TaskStatus::Queued).unwrap_or(self.tasks.len());
        self.tasks.insert(at, t);
        self.updated_at = now;
        Ok(())
    }

    pub fn check_stop(&self, id: &str) -> Result<(), String> {
        self.require(id, Op::Action(TaskAction::Stop))?;
        Ok(())
    }
}
