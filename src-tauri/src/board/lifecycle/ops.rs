//! Le transizioni viste dalla board: ogni metodo trova il task, applica
//! l'evento con `Task::apply` e aggiorna `Board::updated_at` quando serve.

use super::{LifecycleError, Op, Outcome, TaskEvent};
use crate::board::{Board, Task, TaskReport, TaskStatus, WorkerActivity, WorktreeInfo};

impl Board {
    pub(in crate::board) fn known(&self, id: &str) -> Result<&Task, LifecycleError> {
        self.task(id).ok_or_else(|| LifecycleError::NotFound(id.to_string()))
    }

    /// Il task `id`, se `op` e' permessa nel suo stato (vedi `permits`).
    pub(in crate::board) fn require(&self, id: &str, op: Op) -> Result<&Task, LifecycleError> {
        let t = self.known(id)?;
        if super::permits(t.status(), op) {
            Ok(t)
        } else {
            Err(LifecycleError::NotAllowed { task: t.id.clone(), status: t.status(), op })
        }
    }

    fn apply(&mut self, id: &str, event: TaskEvent, now: i64) -> Result<(), LifecycleError> {
        self.task_mut(id).ok_or_else(|| LifecycleError::NotFound(id.to_string()))?.apply(event, now)
    }

    /// Applica l'evento e, se riesce, aggiorna la data della board.
    fn apply_touching(&mut self, id: &str, event: TaskEvent, now: i64) -> Result<(), LifecycleError> {
        self.apply(id, event, now)?;
        self.updated_at = now;
        Ok(())
    }

    /// Archiviazione del workspace: i task `running` (in avvio o col worker
    /// vivo) tornano in coda, staccati da sessione e preparazione. Ritorna
    /// quanti.
    pub fn requeue_running(&mut self, note: &str, now: i64) -> usize {
        let mut n = 0;
        for t in self.tasks.iter_mut().filter(|t| t.status() == TaskStatus::Running) {
            if t.apply(TaskEvent::Requeue { note: note.to_string() }, now).is_ok() {
                n += 1;
            }
        }
        if n > 0 {
            self.updated_at = now;
        }
        n
    }

    /// "Riprova": `failed` -> `queued` in fondo alla coda. Worktree e branch
    /// restano; l'esito precedente passa al prossimo worker (`previous_attempt`).
    pub fn retry_task(&mut self, id: &str, now: i64) -> Result<(), String> {
        self.apply_touching(id, TaskEvent::Retry, now)?;
        if let Some(pos) = self.tasks.iter().position(|x| x.id == id) {
            let t = self.tasks.remove(pos);
            self.tasks.push(t);
        }
        Ok(())
    }

    /// "Segna come fatto": `failed` -> `done` a mano, per un lavoro consegnato
    /// ma finito fallito. Il report resta; la nota del fallimento e il tentativo
    /// precedente no. Worktree e branch non si toccano.
    pub fn mark_done(&mut self, id: &str, now: i64) -> Result<(), String> {
        self.apply_touching(id, TaskEvent::MarkDone, now)?;
        Ok(())
    }

    /// Segna `running` un task scelto dallo scheduler. Mantiene
    /// `previous_attempt` (lo legge `get_task`).
    pub fn start_task(&mut self, id: &str, now: i64) -> Result<(), LifecycleError> {
        self.apply_touching(id, TaskEvent::Start, now)
    }

    /// Segno di vita della preparazione di un avvio ancora in corso (task
    /// `running` senza sessione, stesso tentativo). `label` = passo da mostrare
    /// (`None` = solo battito: il guardiano non scatta). `false` se l'avvio non
    /// e' piu' quello.
    pub fn touch_preparing(&mut self, id: &str, attempt: u32, label: Option<&str>, now: i64) -> bool {
        let event = TaskEvent::PrepareStep { attempt, step: label.map(str::to_string) };
        self.apply(id, event, now).is_ok()
    }

    /// Fine della preparazione (setup finito o fallito): via l'etichetta, il
    /// battito resta a `now`.
    pub fn clear_preparing(&mut self, id: &str, attempt: u32, now: i64) -> bool {
        self.apply(id, TaskEvent::PrepareEnded { attempt }, now).is_ok()
    }

    /// Registra il worktree appena creato, prima del setup: se il setup
    /// fallisce, il retry lo ritrova (con `setup_done` falso) e lo rifa.
    pub fn record_worktree(&mut self, id: &str, attempt: u32, wt: WorktreeInfo) -> bool {
        // Non cambia `updated_at`: l'istante non serve.
        self.apply(id, TaskEvent::WorktreeCreated { attempt, worktree: wt }, 0).is_ok()
    }

    /// Il worker `session` e' stato spawnato per il tentativo `attempt`.
    pub fn worker_spawned(
        &mut self,
        id: &str,
        attempt: u32,
        session: String,
        worktree: Option<WorktreeInfo>,
        note: Option<String>,
        now: i64,
    ) -> bool {
        self.apply(id, TaskEvent::WorkerSpawned { attempt, session, worktree, note }, now).is_ok()
    }

    /// Il worker `session` cambia attivita'. `false` se non e' il worker del task.
    pub fn worker_changed(&mut self, id: &str, session: &str, activity: WorkerActivity, now: i64) -> bool {
        self.apply(id, TaskEvent::WorkerChanged { session: session.to_string(), activity }, now).is_ok()
    }

    /// Fine di un task `running` (report del worker, avvio fallito, tab
    /// chiuso, stop). Se `session` e' dato deve essere quella del task.
    /// `false` se il task non era piu' `running` per quel worker.
    pub fn finish_running(
        &mut self,
        id: &str,
        session: Option<&str>,
        outcome: Outcome,
        report: Option<TaskReport>,
        note: Option<String>,
        now: i64,
    ) -> bool {
        let report = report.map(Box::new);
        let event = TaskEvent::Finish { session: session.map(str::to_string), outcome, report, note };
        self.apply_touching(id, event, now).is_ok()
    }

    /// Il worker `session` fa una domanda all'utente. Ritorna l'id della domanda.
    /// `Stale` se non e' il worker vivo del task o c'e' gia' una domanda in attesa.
    pub fn ask_question(
        &mut self,
        id: &str,
        session: &str,
        text: String,
        options: Vec<String>,
        now: i64,
    ) -> Result<String, LifecycleError> {
        let event = TaskEvent::QuestionAsked { session: session.to_string(), text, options };
        self.apply_touching(id, event, now)?;
        let task = self.known(id)?;
        task.questions().last().map(|q| q.id.clone()).ok_or(LifecycleError::Stale)
    }

    /// La domanda in attesa `question_id` del worker `session` ha avuto risposta
    /// (`answer` = testo consegnato dall'app, `None` = risposta data nel terminale).
    pub fn answer_question(
        &mut self,
        id: &str,
        session: &str,
        question_id: &str,
        answer: Option<String>,
        now: i64,
    ) -> Result<(), LifecycleError> {
        let event =
            TaskEvent::QuestionAnswered { session: session.to_string(), question_id: question_id.to_string(), answer };
        self.apply_touching(id, event, now)
    }

    /// Annulla `answer_question` quando la consegna nel terminale e' fallita:
    /// `false` se la risposta non e' piu' quella dell'ultima domanda del worker.
    pub fn answer_undelivered(&mut self, id: &str, session: &str, question_id: &str, now: i64) -> bool {
        let event = TaskEvent::AnswerUndelivered { session: session.to_string(), question_id: question_id.to_string() };
        self.apply_touching(id, event, now).is_ok()
    }

    /// Riapre per `session` un task fallito perche' la sua sessione era finita
    /// al tentativo `attempt`: il worker che lo aveva era ancora vivo e
    /// riferisce. Solo se nessuno l'ha ripreso nel frattempo (ancora `failed`
    /// per quella sessione e quel tentativo). Il task torna `running` con
    /// `note` come traccia; la conclusione resta a `finish_running`.
    pub fn reopen_failed(&mut self, id: &str, session: &str, attempt: u32, note: &str, now: i64) -> bool {
        let event = TaskEvent::Reopen { session: session.to_string(), attempt, note: note.to_string() };
        self.apply_touching(id, event, now).is_ok()
    }
}
