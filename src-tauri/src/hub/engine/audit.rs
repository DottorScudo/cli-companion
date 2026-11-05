//! Traccia e limite delle modifiche di un turno, decisi dalla classe dei tool
//! (`SecretaryTool::class`), puri:
//!
//! - **audit**: una chiamata riuscita che modifica qualcosa lascia un avviso di
//!   sistema nel thread (`notice`), tranne quando il suo effetto e' gia'
//!   visibile li'. Un tool nuovo che modifica dichiara solo il suo testo;
//! - **limite**: al massimo `MAX_MUTATIONS_PER_TURN` modifiche riuscite per
//!   turno (`MutationBudget`); oltre, la chiamata e' rifiutata come un tool
//!   non consentito.

use super::texts;
use super::tools::{Invocation, ToolRejection};
use crate::hub::settings::{SecretaryTool, ToolClass};
use serde_json::Value;

/// Modifiche riuscite per turno, oltre a `post_message`.
pub const MAX_MUTATIONS_PER_TURN: usize = 3;

/// Se una chiamata riuscita del tool conta nel limite del turno: ogni modifica
/// tranne `post_message`, che scrive solo un messaggio gia' visibile.
pub fn counts_toward_limit(tool: SecretaryTool) -> bool {
    match tool.class() {
        ToolClass::Read => false,
        ToolClass::Write => tool != SecretaryTool::PostMessage,
        ToolClass::Destructive => true,
    }
}

/// Le modifiche riuscite di un turno.
#[derive(Debug, Default)]
pub struct MutationBudget {
    used: usize,
}

impl MutationBudget {
    /// Se il turno puo' ancora eseguire `tool`.
    pub fn admit(&self, tool: SecretaryTool) -> Result<(), ToolRejection> {
        if counts_toward_limit(tool) && self.used >= MAX_MUTATIONS_PER_TURN {
            return Err(ToolRejection::TurnLimit(tool));
        }
        Ok(())
    }

    /// Registra una chiamata riuscita di `tool`.
    pub fn record(&mut self, tool: SecretaryTool) {
        if counts_toward_limit(tool) {
            self.used += 1;
        }
    }
}

/// L'avviso (in italiano) che una chiamata riuscita lascia nel thread, o
/// `None` se non modifica nulla o se il suo effetto si vede gia' nel thread.
pub fn notice(secretary: &str, invocation: &Invocation, outcome: &Value) -> Option<String> {
    match invocation {
        Invocation::BoardStatus { .. }
        | Invocation::BoardOverview
        | Invocation::TaskDetails { .. }
        | Invocation::ThreadRead { .. } => None,
        // Gia' visibili: il messaggio stesso, l'evento del task creato, gli
        // eventi della proposta decisa, la scheda della richiesta di conferma.
        Invocation::PostMessage { .. }
        | Invocation::OpenPlanTask { .. }
        | Invocation::DecideProposal { .. }
        | Invocation::RequestTaskAction { .. } => None,
        Invocation::RelayAnswer { body, .. } => {
            Some(texts::answer_relayed(secretary, text_of(outcome, "task_id"), body))
        }
        Invocation::TaskAction { op, .. } => {
            Some(texts::task_changed(secretary, op, text_of(outcome, "title"), text_of(outcome, "before_title")))
        }
        // Solo cio' che e' cambiato davvero (`board_ops::control_board`).
        Invocation::BoardControl { .. } => texts::board_changed(
            secretary,
            text_of(outcome, "workspace"),
            outcome["changed"]["paused"].as_bool(),
            outcome["changed"]["max_parallel"].as_u64(),
        ),
    }
}

/// Un campo testuale dell'esito, vuoto se manca.
fn text_of<'a>(outcome: &'a Value, field: &str) -> &'a str {
    outcome[field].as_str().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::TaskAction;
    use crate::hub::engine::board_ops::{BoardChange, TaskOp};
    use crate::hub::engine::board_query::BoardQuery;
    use crate::hub::engine::relay::QuestionSelector;
    use serde_json::json;

    /// Una chiamata valida per ogni tool, nell'ordine canonico.
    fn one_of_each() -> [Invocation; 11] {
        [
            Invocation::BoardStatus { workspace: None, query: BoardQuery::default() },
            Invocation::BoardOverview,
            Invocation::TaskDetails { workspace: None, task_id: "t-1".into() },
            Invocation::ThreadRead { page: Default::default() },
            Invocation::PostMessage { body: "Controllo.".into() },
            Invocation::OpenPlanTask { goal: "Add a login page".into() },
            Invocation::DecideProposal { accept: true },
            Invocation::RelayAnswer { body: "Use Postgres.".into(), selector: QuestionSelector::default() },
            Invocation::TaskAction { workspace: None, task_id: "t-7".into(), op: TaskOp::Retry },
            Invocation::BoardControl {
                workspace: None,
                change: BoardChange { paused: Some(true), max_parallel: None },
            },
            Invocation::RequestTaskAction {
                task_id: "t-7".into(),
                action: TaskAction::Delete,
                reason: "Obsolete".into(),
            },
        ]
    }

    #[test]
    fn invocations_name_their_tool() {
        assert_eq!(one_of_each().map(|i| i.tool()), SecretaryTool::ALL);
    }

    #[test]
    fn only_changes_other_than_post_message_count() {
        let counted: Vec<_> = SecretaryTool::ALL.into_iter().filter(|t| counts_toward_limit(*t)).collect();
        assert_eq!(
            counted,
            [
                SecretaryTool::OpenPlanTask,
                SecretaryTool::DecideProposal,
                SecretaryTool::RelayAnswer,
                SecretaryTool::TaskAction,
                SecretaryTool::BoardControl,
                SecretaryTool::RequestTaskAction
            ]
        );
    }

    #[test]
    fn the_budget_refuses_the_change_past_the_limit() {
        let mut budget = MutationBudget::default();
        for _ in 0..MAX_MUTATIONS_PER_TURN {
            assert_eq!(budget.admit(SecretaryTool::OpenPlanTask), Ok(()));
            budget.record(SecretaryTool::OpenPlanTask);
        }
        assert_eq!(
            budget.admit(SecretaryTool::DecideProposal),
            Err(ToolRejection::TurnLimit(SecretaryTool::DecideProposal))
        );
        // Letture e messaggi restano liberi, e non consumano il limite.
        for tool in [
            SecretaryTool::BoardStatus,
            SecretaryTool::BoardOverview,
            SecretaryTool::TaskDetails,
            SecretaryTool::ThreadRead,
            SecretaryTool::PostMessage,
        ] {
            assert_eq!(budget.admit(tool), Ok(()));
        }
        let mut fresh = MutationBudget::default();
        (0..10).for_each(|_| fresh.record(SecretaryTool::PostMessage));
        assert_eq!(fresh.admit(SecretaryTool::RelayAnswer), Ok(()));
    }

    #[test]
    fn only_changes_not_visible_in_the_thread_leave_a_notice() {
        assert_eq!(
            one_of_each().map(|i| notice("mario", &i, &outcome())),
            [
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some("@mario ha inviato al worker del task t-7 questa risposta:\nUse Postgres.".into()),
                Some("@mario ha ritentato «Login».".into()),
                Some("@mario ha messo in pausa la board di Sito Web.".into()),
                None,
            ]
        );
    }

    /// Un esito con i campi letti dagli avvisi.
    fn outcome() -> Value {
        json!({ "task_id": "t-7", "title": "Login", "workspace": "Sito Web", "changed": { "paused": true } })
    }

    #[test]
    fn a_request_for_confirmation_counts_toward_the_limit_but_its_card_is_the_audit() {
        assert!(counts_toward_limit(SecretaryTool::RequestTaskAction));
        let mut budget = MutationBudget::default();
        for _ in 0..MAX_MUTATIONS_PER_TURN {
            budget.record(SecretaryTool::RequestTaskAction);
        }
        assert_eq!(
            budget.admit(SecretaryTool::RequestTaskAction),
            Err(ToolRejection::TurnLimit(SecretaryTool::RequestTaskAction))
        );
    }

    #[test]
    fn a_board_control_that_changed_nothing_leaves_no_notice() {
        let control = &one_of_each()[9];
        let unchanged = json!({ "workspace": "Sito Web", "paused": true, "changed": {} });
        assert_eq!(notice("mario", control, &unchanged), None);
        let parallel = json!({ "workspace": "Sito Web", "changed": { "max_parallel": 2 } });
        assert_eq!(
            notice("mario", control, &parallel).unwrap(),
            "@mario ha impostato a 2 i task in parallelo della board di Sito Web."
        );
    }
}
