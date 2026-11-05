use super::*;
use crate::hub::engine::audit::MAX_MUTATIONS_PER_TURN;
use crate::hub::engine::board_ops::{BoardChange, TaskOp};
use crate::hub::engine::fake::{call, calls, text, FakeModel};
use crate::hub::engine::fixtures::{secretary, FakeBoard, WS};
use crate::hub::engine::planning::{self, ProposalBoard, ProposalRef};
use crate::hub::engine::relay::{self, relay_candidates, QuestionSelector, RelayCandidates};
use crate::hub::engine::sessions::WakeCause;
use crate::hub::engine::thread_page::ThreadPage;
use crate::hub::model::HubEventKind;
use crate::hub::model::RequestedAction;
use crate::hub::questions::fixtures::{question_event, question_ref, task_event, thread, FakeAnswers};
use crate::hub::settings::SecretaryTool;
use std::sync::Mutex;
use tauri::async_runtime::block_on;

/// Host finto: board e thread fissi, registra messaggi, avvisi e piani;
/// `decide_proposal` passa dalla regola vera (`planning::decide`) su una board
/// finta, con la proposta che il turno puo' decidere.
#[derive(Default)]
struct FakeHost {
    posted: Mutex<Vec<String>>,
    notices: Mutex<Vec<String>>,
    boards: Mutex<Vec<Option<String>>>,
    overviews: Mutex<usize>,
    details: Mutex<Vec<(Option<String>, String)>>,
    pages: Mutex<Vec<ThreadPage>>,
    plans: Mutex<Vec<String>>,
    proposal: Option<ProposalRef>,
    board: FakeBoard,
    /// Le domande a cui il turno puo' rispondere (`relay::relay_candidates`).
    question: Option<RelayCandidates>,
    answers: FakeAnswers,
    task_actions: Mutex<Vec<(Option<String>, String, TaskOp)>>,
    board_changes: Mutex<Vec<(Option<String>, BoardChange)>>,
    requests: Mutex<Vec<(String, RequestedAction, String)>>,
    updates: Mutex<Vec<(String, Value, String)>>,
    states: Mutex<Vec<ThreadState>>,
}

impl FakeHost {
    /// Il turno risponde alla proposta pendente `p-1` (2 task).
    fn replying_to_proposal() -> Self {
        Self {
            proposal: Some(ProposalRef { workspace: WS.into(), proposal_id: "p-1".into() }),
            board: FakeBoard::with_pending(WS, "p-1", 2),
            ..Self::default()
        }
    }
}

impl TurnHost for FakeHost {
    async fn board_status(&self, workspace: Option<String>, _query: BoardQuery) -> Result<Value, String> {
        self.boards.lock().unwrap().push(workspace.clone());
        match workspace.as_deref() {
            Some("gone") => Err("Workspace `gone` not found.".to_string()),
            _ => Ok(json!({ "tasks": [{ "id": "t-1", "status": "running" }] })),
        }
    }

    async fn board_overview(&self) -> Result<Value, String> {
        *self.overviews.lock().unwrap() += 1;
        Ok(json!({ "workspaces": [] }))
    }

    async fn task_details(&self, workspace: Option<String>, task_id: String) -> Result<Value, String> {
        self.details.lock().unwrap().push((workspace, task_id.clone()));
        match task_id.as_str() {
            "t-404" => Err("No task with id \"t-404\" on the board of workspace \"Muse Companion\".".to_string()),
            _ => Ok(json!({ "id": task_id, "status": "done", "report": { "commit": "abc1234" } })),
        }
    }

    async fn thread_read(&self, page: ThreadPage) -> Result<Value, String> {
        self.pages.lock().unwrap().push(page);
        Ok(json!({ "title": "Login", "big": "x".repeat(TOOL_RESULT_MAX * 2) }))
    }

    async fn post_message(&self, body: String) -> Result<(), String> {
        self.posted.lock().unwrap().push(body);
        Ok(())
    }

    async fn open_plan_task(&self, goal: String) -> Result<Value, String> {
        self.plans.lock().unwrap().push(goal);
        Ok(json!({ "task_id": "plan-login", "status": "queued" }))
    }

    async fn decide_proposal(&self, accept: bool) -> Result<Value, String> {
        planning::decide(&self.board, self.proposal.as_ref(), accept)
    }

    async fn relay_answer(&self, selector: QuestionSelector, body: String) -> Result<Value, String> {
        let target = relay::choose(&self.question.clone().unwrap_or(Err("no question for this turn")), &selector);
        relay::relay(&self.answers, target.as_ref().map_err(|e| *e), &body)
    }

    async fn task_action(&self, workspace: Option<String>, task_id: String, op: TaskOp) -> Result<Value, String> {
        self.task_actions.lock().unwrap().push((workspace, task_id.clone(), op));
        match task_id.as_str() {
            "t-404" => Err("No task with id \"t-404\" on the board of workspace \"Muse Companion\".".to_string()),
            _ => Ok(json!({ "task_id": task_id, "title": "Login", "status": "queued" })),
        }
    }

    async fn board_control(&self, workspace: Option<String>, change: BoardChange) -> Result<Value, String> {
        self.board_changes.lock().unwrap().push((workspace, change));
        Ok(json!({ "workspace": "Muse Companion", "changed": { "paused": change.paused } }))
    }

    async fn request_task_action(
        &self,
        task_id: String,
        action: RequestedAction,
        reason: String,
    ) -> Result<Value, String> {
        self.requests.lock().unwrap().push((task_id.clone(), action, reason));
        Ok(json!({ "request_id": "msg-1", "task_id": task_id, "status": "awaiting_confirmation" }))
    }

    async fn request_task_update(&self, task_id: String, patch: Value, reason: String) -> Result<Value, String> {
        self.updates.lock().unwrap().push((task_id.clone(), patch, reason));
        Ok(json!({ "request_id": "msg-1", "task_id": task_id, "status": "awaiting_confirmation" }))
    }

    async fn set_thread_state(&self, state: ThreadState) -> Result<Value, String> {
        let mut states = self.states.lock().unwrap();
        // Il thread del finto host parte aperto: richiudere o riaprire a vuoto non cambia nulla.
        let before = states.last().copied().unwrap_or(ThreadState::Open);
        states.push(state);
        Ok(json!({ "state": state, "changed": before != state }))
    }

    async fn notice(&self, text: String) {
        self.notices.lock().unwrap().push(text);
    }
}

/// L'esito del tool restituito al modello nella richiesta `request`.
fn tool_result(model: &FakeModel, request: usize) -> String {
    let requests = model.requests();
    match requests[request].messages.last() {
        Some(ChatMessage::Tool { content, .. }) => content.clone(),
        other => panic!("nessun esito di tool: {other:?}"),
    }
}

fn spec(s: &Secretary, max_steps: u32) -> TurnSpec<'_> {
    TurnSpec { secretary: s, system: "system".into(), max_steps }
}

fn run(model: &FakeModel, host: &FakeHost, s: &Secretary, max_steps: u32) -> (TurnEnd, Vec<ChatMessage>) {
    let mut conversation = vec![ChatMessage::User("@mario a che punto siamo?".into())];
    let end = block_on(run_turn(model, host, &spec(s, max_steps), &mut conversation));
    (end, conversation)
}

#[test]
fn a_text_reply_ends_the_turn() {
    let model = FakeModel::new(vec![text("Il task t-1 è in corso.")]);
    let host = FakeHost::default();
    let (end, conversation) = run(&model, &host, &secretary("mario"), 8);
    assert_eq!(end, TurnEnd::Reply("Il task t-1 è in corso.".into()));
    assert_eq!(conversation.len(), 2);
    let requests = model.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].system, "system");
    assert_eq!(
        requests[0].tools,
        [
            "board_status",
            "board_overview",
            "task_details",
            "thread_read",
            "post_message",
            "open_plan_task",
            "decide_proposal",
            "relay_answer",
            "task_action",
            "board_control",
            "request_task_action",
            "request_task_update",
            "set_thread_state"
        ]
    );
}

#[test]
fn tools_run_and_their_results_go_back_to_the_model() {
    let model = FakeModel::new(vec![
        calls(vec![call("c1", "board_status", "{}"), call("c2", "post_message", r#"{"body":"Controllo la board."}"#)]),
        calls(vec![call("c3", "thread_read", "")]),
        text("t-1 è in corso."),
    ]);
    let host = FakeHost::default();
    let (end, conversation) = run(&model, &host, &secretary("mario"), 8);
    assert_eq!(end, TurnEnd::Reply("t-1 è in corso.".into()));
    assert_eq!(*host.posted.lock().unwrap(), ["Controllo la board."]);
    assert_eq!(*host.boards.lock().unwrap(), [None]);
    assert!(host.notices.lock().unwrap().is_empty());
    let requests = model.requests();
    assert_eq!(requests.len(), 3);
    let second = &requests[1].messages;
    assert!(matches!(&second[1], ChatMessage::Assistant { tool_calls, .. } if tool_calls.len() == 2));
    assert!(matches!(&second[2], ChatMessage::Tool { call_id, content } if call_id == "c1" && content.contains("t-1")));
    assert!(matches!(&second[3], ChatMessage::Tool { content, .. } if content == r#"{"posted":true}"#));
    // L'esito lungo di thread_read e' tagliato.
    let ChatMessage::Tool { content, .. } = &conversation[5] else { panic!("{conversation:?}") };
    assert_eq!(content.chars().count(), TOOL_RESULT_MAX + 1);
}

#[test]
fn a_tool_outside_the_list_is_refused_with_a_notice_and_the_turn_goes_on() {
    let mut s = secretary("mario");
    s.tools = vec![SecretaryTool::ThreadRead];
    let model =
        FakeModel::new(vec![calls(vec![call("c1", "board_status", "{}")]), text("Non posso leggere la board.")]);
    let host = FakeHost::default();
    let (end, _) = run(&model, &host, &s, 8);
    assert_eq!(end, TurnEnd::Reply("Non posso leggere la board.".into()));
    assert!(host.boards.lock().unwrap().is_empty());
    let notices = host.notices.lock().unwrap();
    assert_eq!(notices.len(), 1);
    assert!(notices[0].contains("«board_status»"), "{notices:?}");
    let back = &model.requests()[1].messages[2];
    assert!(matches!(back, ChatMessage::Tool { content, .. } if content.contains("not available to you")));
    assert_eq!(model.requests()[0].tools, ["thread_read"]);
}

#[test]
fn invalid_arguments_and_tool_errors_return_an_error_to_the_model() {
    let model = FakeModel::new(vec![
        calls(vec![
            call("c1", "post_message", r#"{"text":"x"}"#),
            call("c2", "board_status", r#"{"workspace":"gone"}"#),
        ]),
        text("Scusa."),
    ]);
    let host = FakeHost::default();
    let (end, _) = run(&model, &host, &secretary("mario"), 8);
    assert_eq!(end, TurnEnd::Reply("Scusa.".into()));
    assert!(host.posted.lock().unwrap().is_empty());
    assert_eq!(host.notices.lock().unwrap().len(), 1, "solo il tool rifiutato e' un avviso");
    let messages = &model.requests()[1].messages;
    assert!(
        matches!(&messages[2], ChatMessage::Tool { content, .. } if content.contains("Invalid arguments for `post_message`"))
    );
    assert!(matches!(&messages[3], ChatMessage::Tool { content, .. } if content.contains("not found")));
}

#[test]
fn a_clear_yes_in_reply_to_the_proposal_accepts_it() {
    let model = FakeModel::new(vec![
        calls(vec![call("c1", "decide_proposal", r#"{"accept":true}"#)]),
        text("Fatto: 2 task in coda."),
    ]);
    let host = FakeHost::replying_to_proposal();
    let (end, _) = run(&model, &host, &secretary("giorgio"), 8);
    assert_eq!(end, TurnEnd::Reply("Fatto: 2 task in coda.".into()));
    assert_eq!(*host.board.decisions.lock().unwrap(), [(WS.to_string(), "p-1".to_string(), true)]);
    let result: Value = serde_json::from_str(&tool_result(&model, 1)).unwrap();
    assert_eq!(result, json!({ "proposal": "p-1", "decision": "accepted", "tasks_queued": 2 }));
}

#[test]
fn a_no_in_reply_to_the_proposal_rejects_it() {
    let model = FakeModel::new(vec![
        calls(vec![call("c1", "decide_proposal", r#"{"accept":false}"#)]),
        text("Proposta scartata."),
    ]);
    let host = FakeHost::replying_to_proposal();
    run(&model, &host, &secretary("giorgio"), 8);
    assert_eq!(*host.board.decisions.lock().unwrap(), [(WS.to_string(), "p-1".to_string(), false)]);
}

#[test]
fn an_ambiguous_reply_gets_a_question_and_no_decision() {
    let model = FakeModel::new(vec![text("Vuoi che accetti la proposta o che la scarti?")]);
    let host = FakeHost::replying_to_proposal();
    let (end, _) = run(&model, &host, &secretary("giorgio"), 8);
    assert!(matches!(end, TurnEnd::Reply(q) if q.ends_with('?')));
    assert!(host.board.decisions.lock().unwrap().is_empty());
    assert_eq!(host.board.pending_tasks(WS, "p-1"), Some(2));
}

#[test]
fn deciding_without_a_reply_to_the_proposal_is_refused() {
    // Nessuna proposta per il turno: un messaggio senza reply_to sulla
    // proposta, o un risveglio da evento della board.
    let model = FakeModel::new(vec![
        calls(vec![call("c1", "decide_proposal", r#"{"accept":true}"#)]),
        text("Rispondi al messaggio della proposta per decidere."),
    ]);
    let host = FakeHost { board: FakeBoard::with_pending(WS, "p-1", 2), ..FakeHost::default() };
    let (end, _) = run(&model, &host, &secretary("giorgio"), 8);
    assert!(matches!(end, TurnEnd::Reply(_)));
    assert!(host.board.decisions.lock().unwrap().is_empty());
    assert!(tool_result(&model, 1).contains("decide_proposal works only when"), "{}", tool_result(&model, 1));
    assert!(host.notices.lock().unwrap().is_empty(), "un errore d'esecuzione, non un tool rifiutato");
}

#[test]
fn a_secretary_without_the_planning_tools_cannot_use_them() {
    let mut s = secretary("mario");
    s.tools = vec![SecretaryTool::BoardStatus, SecretaryTool::ThreadRead, SecretaryTool::PostMessage];
    let model = FakeModel::new(vec![
        calls(vec![
            call("c1", "decide_proposal", r#"{"accept":true}"#),
            call("c2", "open_plan_task", r#"{"goal":"Add a login page"}"#),
        ]),
        text("Non posso."),
    ]);
    let host = FakeHost::replying_to_proposal();
    run(&model, &host, &s, 8);
    assert!(host.board.decisions.lock().unwrap().is_empty() && host.plans.lock().unwrap().is_empty());
    assert_eq!(host.notices.lock().unwrap().len(), 2);
    assert_eq!(model.requests()[0].tools, ["board_status", "thread_read", "post_message"]);
}

/// Un turno che chiama `relay_answer` con `body` e poi risponde, con la
/// domanda ricavata dal thread `messages` per un risveglio `cause`.
fn relay_turn(messages: Vec<crate::hub::model::Message>, cause: WakeCause) -> (FakeModel, FakeHost) {
    relay_turn_with(messages, cause, r#"{"body":"Use the second option."}"#)
}

/// Come `relay_turn`, con gli argomenti `arguments` della chiamata.
fn relay_turn_with(
    messages: Vec<crate::hub::model::Message>,
    cause: WakeCause,
    arguments: &str,
) -> (FakeModel, FakeHost) {
    let model = FakeModel::new(vec![calls(vec![call("c1", "relay_answer", arguments)]), text("Fatto.")]);
    let host = FakeHost { question: Some(relay_candidates(&thread(messages), cause)), ..FakeHost::default() };
    run(&model, &host, &secretary("mario"), 8);
    (model, host)
}

/// Due task con la stessa domanda `q-1-1` in attesa, come accade davvero: l'id
/// di una domanda e' unico solo nel suo task.
fn two_pending_questions() -> Vec<crate::hub::model::Message> {
    vec![question_event("e1", "a", "q-1-1"), question_event("e2", "b", "q-1-1")]
}

#[test]
fn relay_answer_delivers_to_the_only_pending_question_when_the_user_asks() {
    let (model, host) = relay_turn(vec![question_event("e1", "a", "q-1-1")], WakeCause::Message);
    assert_eq!(host.answers.delivered(), [(question_ref("a", "q-1-1"), "Use the second option.".to_string())]);
    assert!(tool_result(&model, 1).contains(r#""delivered":true"#), "{}", tool_result(&model, 1));
    assert_eq!(
        *host.notices.lock().unwrap(),
        ["@mario ha inviato al worker del task a questa risposta:\nUse the second option."],
        "l'avviso di audit, una volta sola, con il task"
    );
}

#[test]
fn relay_answer_with_ids_delivers_to_the_right_one_of_several_questions() {
    let args = r#"{"body":"Use the second option.","question_id":"q-1-1","task_id":"b"}"#;
    let (model, host) = relay_turn_with(two_pending_questions(), WakeCause::Message, args);
    assert_eq!(host.answers.delivered(), [(question_ref("b", "q-1-1"), "Use the second option.".to_string())]);
    assert!(tool_result(&model, 1).contains(r#""task_id":"b""#), "{}", tool_result(&model, 1));
    assert_eq!(
        *host.notices.lock().unwrap(),
        ["@mario ha inviato al worker del task b questa risposta:\nUse the second option."]
    );
}

#[test]
fn relay_answer_refuses_an_id_that_is_not_a_pending_question_of_the_thread() {
    let foreign = r#"{"body":"x","question_id":"q-9-9","task_id":"b"}"#;
    let (model, host) = relay_turn_with(two_pending_questions(), WakeCause::Message, foreign);
    assert!(host.answers.delivered().is_empty());
    assert!(host.notices.lock().unwrap().is_empty());
    assert!(tool_result(&model, 1).contains("matches that `question_id`"), "{}", tool_result(&model, 1));
    let answered = vec![question_event("e1", "a", "q-1-1"), task_event("e2", HubEventKind::TaskQuestionAnswered, "a")];
    let (_, host) = relay_turn_with(answered, WakeCause::Message, r#"{"body":"x","question_id":"q-1-1"}"#);
    assert!(host.answers.delivered().is_empty());
}

#[test]
fn relay_answer_with_ids_is_still_refused_when_an_event_or_external_item_woke_the_turn() {
    let args = r#"{"body":"x","question_id":"q-1-1","task_id":"a"}"#;
    for cause in [WakeCause::BoardEvent, WakeCause::External] {
        let (model, host) = relay_turn_with(two_pending_questions(), cause, args);
        assert!(host.answers.delivered().is_empty(), "{cause:?}");
        assert!(tool_result(&model, 1).contains("woke you up"), "{cause:?}: {}", tool_result(&model, 1));
    }
}

#[test]
fn relay_answer_is_refused_in_a_board_event_turn_and_with_several_questions() {
    let (model, host) = relay_turn(vec![question_event("e1", "a", "q-1-1")], WakeCause::BoardEvent);
    assert!(host.answers.delivered().is_empty());
    assert!(host.notices.lock().unwrap().is_empty(), "una modifica fallita non lascia un avviso di audit");
    assert!(tool_result(&model, 1).contains("never when a board event or external content woke you up"));
    let several = vec![question_event("e1", "a", "q-1-1"), question_event("e2", "b", "q-1-1")];
    let (model, host) = relay_turn(several, WakeCause::Message);
    assert!(host.answers.delivered().is_empty());
    assert!(tool_result(&model, 1).contains("ask the user which question"));
    let answered = vec![question_event("e1", "a", "q-1-1"), task_event("e2", HubEventKind::TaskQuestionAnswered, "a")];
    let (model, host) = relay_turn(answered, WakeCause::Message);
    assert!(host.answers.delivered().is_empty());
    assert!(tool_result(&model, 1).contains("No worker question is waiting"));
}

#[test]
fn relay_answer_is_refused_in_an_external_turn_even_with_one_pending_question() {
    let (model, host) = relay_turn(vec![question_event("e1", "a", "q-1-1")], WakeCause::External);
    assert!(host.answers.delivered().is_empty());
    assert!(tool_result(&model, 1).contains("external content woke you up"), "{}", tool_result(&model, 1));
}

#[test]
fn a_secretary_without_relay_answer_cannot_use_it() {
    let mut s = secretary("mario");
    s.tools.retain(|t| *t != SecretaryTool::RelayAnswer);
    let model = FakeModel::new(vec![
        calls(vec![call("c1", "relay_answer", r#"{"body":"Use the second option."}"#)]),
        text("Non posso."),
    ]);
    let host = FakeHost { question: Some(Ok(vec![question_ref("a", "q-1-1")])), ..FakeHost::default() };
    run(&model, &host, &s, 8);
    assert!(host.answers.delivered().is_empty());
    assert_eq!(host.notices.lock().unwrap().len(), 1);
    assert!(!model.requests()[0].tools.contains(&"relay_answer"));
}

#[test]
fn open_plan_task_hands_the_goal_to_the_host() {
    let model = FakeModel::new(vec![
        calls(vec![call("c1", "open_plan_task", r#"{"goal":"Add a login page with email and password"}"#)]),
        text("Ho aperto la pianificazione: ti scrivo quando c'è la proposta."),
    ]);
    let host = FakeHost::default();
    run(&model, &host, &secretary("giorgio"), 8);
    assert_eq!(*host.plans.lock().unwrap(), ["Add a login page with email and password"]);
    assert!(tool_result(&model, 1).contains("plan-login"));
}

#[test]
fn changes_already_visible_in_the_thread_leave_no_audit_notice() {
    let model = FakeModel::new(vec![
        calls(vec![
            call("c1", "post_message", r#"{"body":"Apro la pianificazione."}"#),
            call("c2", "open_plan_task", r#"{"goal":"Add a login page"}"#),
            call("c3", "decide_proposal", r#"{"accept":true}"#),
        ]),
        text("Fatto."),
    ]);
    let host = FakeHost::replying_to_proposal();
    run(&model, &host, &secretary("giorgio"), 8);
    assert_eq!(host.posted.lock().unwrap().len(), 1);
    assert_eq!(host.plans.lock().unwrap().len(), 1);
    assert_eq!(host.board.decisions.lock().unwrap().len(), 1);
    assert!(host.notices.lock().unwrap().is_empty(), "{:?}", host.notices.lock().unwrap());
}

#[test]
fn board_changes_reach_the_host_and_leave_an_audit_notice() {
    let model = FakeModel::new(vec![
        calls(vec![
            call("c1", "task_action", r#"{"task_id":"t-1","action":"retry"}"#),
            call("c2", "board_control", r#"{"paused":true,"workspace":"Sito Web"}"#),
            call("c3", "task_action", r#"{"task_id":"t-404","action":"to_top"}"#),
        ]),
        text("Ho ritentato il task e messo in pausa la board."),
    ]);
    let host = FakeHost::default();
    run(&model, &host, &secretary("ugo"), 8);
    assert_eq!(
        *host.task_actions.lock().unwrap(),
        [(None, "t-1".to_string(), TaskOp::Retry), (None, "t-404".to_string(), TaskOp::ToTop)]
    );
    assert_eq!(
        *host.board_changes.lock().unwrap(),
        [(Some("Sito Web".to_string()), BoardChange { paused: Some(true), max_parallel: None })]
    );
    // Il rifiuto della board torna al modello e non lascia avvisi.
    assert_eq!(
        *host.notices.lock().unwrap(),
        ["@ugo ha ritentato «Login».", "@ugo ha messo in pausa la board di Muse Companion."]
    );
    assert!(tool_result(&model, 1).contains("No task with id"));
}

#[test]
fn a_request_for_confirmation_reaches_the_host_without_a_notice_and_uses_the_budget() {
    let ask = |id: &str| call(id, "request_task_action", r#"{"task_id":"t-1","action":"delete","reason":"Obsoleto"}"#);
    let model =
        FakeModel::new(vec![calls(vec![ask("c1"), ask("c2"), ask("c3"), ask("c4")]), text("Ho chiesto conferma.")]);
    let host = FakeHost::default();
    run(&model, &host, &secretary("ugo"), 8);
    assert_eq!(host.requests.lock().unwrap().len(), MAX_MUTATIONS_PER_TURN);
    assert_eq!(host.requests.lock().unwrap()[0], ("t-1".to_string(), RequestedAction::Delete, "Obsoleto".to_string()));
    // La scheda e' l'audit: solo il quarto, oltre il limite, lascia un avviso.
    assert_eq!(host.notices.lock().unwrap().len(), 1);
    assert!(tool_result(&model, 1).contains("was not run"));
}

#[test]
fn a_request_to_update_a_task_reaches_the_host_with_the_patch_and_leaves_no_notice() {
    let ask = call(
        "c1",
        "request_task_update",
        r#"{"task_id":"t-1","title":"Login page","model":null,"reason":"Clearer title"}"#,
    );
    let model = FakeModel::new(vec![calls(vec![ask]), text("Ho chiesto conferma.")]);
    let host = FakeHost::default();
    run(&model, &host, &secretary("ugo"), 8);
    assert_eq!(
        *host.updates.lock().unwrap(),
        [("t-1".to_string(), json!({ "title": "Login page", "model": null }), "Clearer title".to_string())]
    );
    assert!(host.notices.lock().unwrap().is_empty(), "la scheda e' gia' l'audit");
}

#[test]
fn closing_the_thread_reaches_the_host_and_leaves_a_notice_only_when_it_changed() {
    let state = |id: &str, state: &str| call(id, "set_thread_state", &format!(r#"{{"state":"{state}"}}"#));
    let model = FakeModel::new(vec![
        calls(vec![state("c1", "closed"), state("c2", "closed"), state("c3", "open")]),
        text("Fatto."),
    ]);
    let host = FakeHost::default();
    run(&model, &host, &secretary("ugo"), 8);
    assert_eq!(*host.states.lock().unwrap(), [ThreadState::Closed, ThreadState::Closed, ThreadState::Open]);
    assert_eq!(*host.notices.lock().unwrap(), ["@ugo ha chiuso il thread.", "@ugo ha riaperto il thread."]);
    assert_eq!(tool_result(&model, 1), r#"{"changed":true,"state":"open"}"#);
}

#[test]
fn the_change_past_the_turn_limit_is_refused_with_a_notice() {
    let plan = |id: &str| call(id, "open_plan_task", r#"{"goal":"Add a login page"}"#);
    let model = FakeModel::new(vec![
        // Una modifica fallita (nessuna proposta per il turno) non consuma il limite.
        calls(vec![call("c0", "decide_proposal", r#"{"accept":true}"#), plan("c1"), plan("c2")]),
        calls(vec![plan("c3"), plan("c4"), call("c5", "post_message", r#"{"body":"Ho aperto tre piani."}"#)]),
        text("Ho aperto tre pianificazioni."),
    ]);
    let host = FakeHost::default();
    let (end, _) = run(&model, &host, &secretary("giorgio"), 8);
    assert_eq!(end, TurnEnd::Reply("Ho aperto tre pianificazioni.".into()));
    assert_eq!(host.plans.lock().unwrap().len(), MAX_MUTATIONS_PER_TURN);
    assert_eq!(*host.posted.lock().unwrap(), ["Ho aperto tre piani."], "post_message non conta nel limite");
    assert_eq!(
        *host.notices.lock().unwrap(),
        ["@giorgio ha chiesto «open_plan_task» dopo 3 modifiche in questo turno, il massimo: richiesta rifiutata."]
    );
    let messages = &model.requests()[2].messages;
    let ChatMessage::Tool { call_id, content } = &messages[messages.len() - 2] else { panic!("{messages:?}") };
    assert_eq!(call_id, "c4");
    let error: Value = serde_json::from_str(content).unwrap();
    assert!(error["error"].as_str().unwrap().starts_with("`open_plan_task` was not run"), "{content}");
}

#[test]
fn the_step_limit_stops_the_turn_without_running_the_last_calls() {
    let model = FakeModel::new(vec![
        calls(vec![call("c1", "thread_read", "")]),
        calls(vec![call("c2", "post_message", r#"{"body":"ancora"}"#)]),
    ]);
    let host = FakeHost::default();
    let (end, conversation) = run(&model, &host, &secretary("mario"), 2);
    assert_eq!(end, TurnEnd::StepLimit);
    assert_eq!(model.requests().len(), 2);
    assert!(host.posted.lock().unwrap().is_empty());
    // L'ultima conversazione valida: utente, chiamata, esito.
    assert_eq!(conversation.len(), 3);
    assert!(matches!(conversation.last(), Some(ChatMessage::Tool { .. })));
}

#[test]
fn endpoint_errors_end_the_turn_with_the_error() {
    for error in [ModelError::Unreachable, ModelError::Timeout, ModelError::Http(500), ModelError::Empty] {
        let model = FakeModel::new(vec![Err(error)]);
        let (end, conversation) = run(&model, &FakeHost::default(), &secretary("mario"), 8);
        assert_eq!(end, TurnEnd::Failed(error));
        assert_eq!(conversation.len(), 1);
    }
    let model = FakeModel::new(vec![calls(vec![call("c1", "thread_read", "")]), Err(ModelError::Timeout)]);
    let (end, conversation) = run(&model, &FakeHost::default(), &secretary("mario"), 8);
    assert_eq!(end, TurnEnd::Failed(ModelError::Timeout));
    assert!(matches!(conversation.last(), Some(ChatMessage::Tool { .. })));
}

#[test]
fn compaction_shortens_old_tool_results_and_keeps_a_valid_start() {
    let mut conversation = Vec::new();
    for i in 0..CONVERSATION_MAX {
        conversation.push(ChatMessage::User(format!("u{i}")));
        conversation.push(ChatMessage::Assistant { text: None, tool_calls: vec![call("c", "thread_read", "")] });
        conversation.push(ChatMessage::Tool { call_id: "c".into(), content: "x".repeat(TOOL_RESULT_KEPT * 3) });
    }
    compact(&mut conversation);
    assert!(conversation.len() <= CONVERSATION_MAX);
    assert!(matches!(&conversation[0], ChatMessage::User(_)));
    assert!(matches!(conversation.last(), Some(ChatMessage::Tool { .. })));
    for m in &conversation {
        if let ChatMessage::Tool { content, .. } = m {
            assert_eq!(content.chars().count(), TOOL_RESULT_KEPT + 1);
        }
    }
    let mut short = vec![ChatMessage::User("x".into())];
    compact(&mut short);
    assert_eq!(short.len(), 1);
}

#[test]
fn the_summary_comes_from_the_model_or_is_kept() {
    let conversation = [ChatMessage::User("stato?".into())];
    let model = FakeModel::new(vec![text("  t-1 running.\n\nUser waits for the login.  ")]);
    let summary = block_on(updated_summary(&model, Some("Old."), &conversation));
    assert_eq!(summary.as_deref(), Some("t-1 running.\nUser waits for the login."));
    let request = &model.requests()[0];
    assert!(request.tools.is_empty());
    assert!(matches!(&request.messages[0], ChatMessage::User(t) if t.contains("Old.") && t.contains("stato?")));
    for reply in [Err(ModelError::Unreachable), calls(vec![call("c", "thread_read", "")]), text("   ")] {
        assert_eq!(block_on(updated_summary(&FakeModel::new(vec![reply]), Some("Old."), &conversation)), None);
    }
}

#[test]
fn the_read_tools_reach_the_host_without_notices_and_without_using_the_change_limit() {
    let plans = (0..MAX_MUTATIONS_PER_TURN).map(|i| call(&format!("p{i}"), "open_plan_task", r#"{"goal":"Add X"}"#));
    let reads = vec![
        call("r1", "board_overview", ""),
        call("r2", "task_details", r#"{"task_id":"t-1","workspace":"Sito Web"}"#),
        call("r3", "thread_read", r#"{"before":"m9"}"#),
        call("r4", "thread_read", r#"{"offset":20}"#),
    ];
    let model = FakeModel::new(vec![calls(plans.chain(reads).collect()), text("Fatto.")]);
    let host = FakeHost::default();
    let (end, _) = run(&model, &host, &secretary("mario"), 8);
    assert_eq!(end, TurnEnd::Reply("Fatto.".into()));
    assert_eq!(*host.overviews.lock().unwrap(), 1);
    assert_eq!(*host.details.lock().unwrap(), [(Some("Sito Web".to_string()), "t-1".to_string())]);
    assert_eq!(*host.pages.lock().unwrap(), [ThreadPage::Before("m9".into()), ThreadPage::Offset(20)]);
    assert!(host.notices.lock().unwrap().is_empty());
    let messages = &model.requests()[1].messages;
    let result = |i: usize| match &messages[i] {
        ChatMessage::Tool { content, .. } => content.clone(),
        other => panic!("{other:?}"),
    };
    assert!(result(2 + MAX_MUTATIONS_PER_TURN + 1).contains("abc1234"));
}

#[test]
fn a_task_that_does_not_exist_is_an_error_for_the_model_and_not_a_notice() {
    let model =
        FakeModel::new(vec![calls(vec![call("c1", "task_details", r#"{"task_id":"t-404"}"#)]), text("Non c'è.")]);
    let host = FakeHost::default();
    let (end, _) = run(&model, &host, &secretary("mario"), 8);
    assert_eq!(end, TurnEnd::Reply("Non c'è.".into()));
    assert!(tool_result(&model, 1).contains(r#""error":"No task with id"#));
    assert!(host.notices.lock().unwrap().is_empty());
}

#[test]
fn a_read_tool_the_secretary_does_not_have_is_refused_with_a_notice() {
    let mut s = secretary("mario");
    s.tools = vec![SecretaryTool::BoardStatus, SecretaryTool::ThreadRead];
    let model = FakeModel::new(vec![
        calls(vec![call("c1", "task_details", r#"{"task_id":"t-1"}"#), call("c2", "board_overview", "{}")]),
        text("Non posso."),
    ]);
    let host = FakeHost::default();
    run(&model, &host, &s, 8);
    assert!(host.details.lock().unwrap().is_empty() && *host.overviews.lock().unwrap() == 0);
    assert_eq!(host.notices.lock().unwrap().len(), 2);
}

#[test]
fn invalid_dates_and_pages_never_reach_the_host() {
    let model = FakeModel::new(vec![
        calls(vec![
            call("c1", "board_status", r#"{"updated_since":"yesterday"}"#),
            call("c2", "thread_read", r#"{"before":"m1","offset":2}"#),
        ]),
        text("Scusa."),
    ]);
    let host = FakeHost::default();
    run(&model, &host, &secretary("mario"), 8);
    assert!(host.boards.lock().unwrap().is_empty() && host.pages.lock().unwrap().is_empty());
    let messages = &model.requests()[1].messages;
    assert!(
        matches!(&messages[2], ChatMessage::Tool { content, .. } if content.contains("`updated_since` must be an ISO 8601"))
    );
    assert!(matches!(&messages[3], ChatMessage::Tool { content, .. } if content.contains("only one of")));
}
