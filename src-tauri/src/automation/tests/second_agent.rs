//! Un secondo agente abilitato (`FakeIntegration` nel registro di test di
//! `agents::test_registry`) usato end-to-end con l'host finto: la validazione
//! dei task, lo scheduler, l'avvio dei worker e dei revisori, gli avvii, le
//! impostazioni del workspace e il caso dell'agente disabilitato passano tutti
//! dal registro, senza scorciatoie nella board.

use super::*;
use crate::agents::builtin_launch_list;
use crate::agents::registry;
use crate::agents::test_registry::SecondIntegration;
use crate::agents::testing::{FakeIntegration, FAKE_ENV_NAME, FAKE_MODEL_LARGE, FAKE_MODEL_SMALL};

const FAKE_ID: &str = "fake-second";
/// Nome dell'agente finto negli errori.
const FAKE_NAME: &str = "Fake";
/// Modello che il catalogo dell'agente finto non accetta.
const FOREIGN_MODEL: &str = "sonnet";
/// Modello che le impostazioni del workspace danno all'agente reale.
const REAL_WORKSPACE_MODEL: &str = "opus";

fn second() -> SecondIntegration {
    SecondIntegration::enabled(FakeIntegration::full(FAKE_ID))
}

fn real_id() -> &'static str {
    registry::default_agent().id()
}

/// Host con la consegna in place e l'agente del workspace dato.
fn host(workspace_agent: Option<&str>) -> (AutomationState, FakeHost) {
    let h = FakeHost::with_delivery(DeliveryMode::Commit, false);
    let mut w = h.ws_for("ws-1").unwrap();
    w.agent = workspace_agent.map(str::to_string);
    h.store_ws1(w);
    (AutomationState::default(), h)
}

/// Specifica con un modello dell'agente finto (le proposte ne vogliono uno).
fn fake_spec(id: &str, deps: &[&str]) -> Value {
    let mut s = spec(id, deps);
    s["model"] = json!(FAKE_MODEL_LARGE);
    s
}

fn agent_of(st: &AutomationState, h: &FakeHost, id: &str) -> String {
    view(st, h).tasks.iter().find(|t| t.task.id == id).unwrap().task.agent.clone()
}

fn status(st: &AutomationState, h: &FakeHost, id: &str) -> String {
    task(st, h, id)["status"].as_str().unwrap().to_string()
}

/// Agente e modello del comando con cui e' partito il worker del task `id`.
fn launched(st: &AutomationState, h: &FakeHost, id: &str) -> (String, String) {
    let session = session_of(st, h, id);
    let spawn = h.spawns().into_iter().find(|s| s.tab_id == session).expect("worker avviato");
    let agent = spawn.command.split_whitespace().nth(1).unwrap().to_string();
    let model = spawn.command.rsplit("model=").next().unwrap().to_string();
    (agent, model)
}

fn set_workspace_model(h: &FakeHost, agent: &str, model: &str) {
    let settings = AgentWorkerSettings { model: Some(model.into()), ..Default::default() };
    h.worker_agents.lock().unwrap().insert(agent.to_string(), settings);
}

/// Task creato dall'interfaccia senza agente e senza modello.
fn create_bare(st: &AutomationState, h: &FakeHost, id: &str) -> Result<BoardView, String> {
    boards::task_create(st, h, "ws-1", &json!({"id": id, "title": "T", "task": "x"}))
}

#[test]
fn a_proposed_task_takes_the_workspace_agent_and_its_worker_starts_with_it() {
    let _second = second();
    let (st, h) = host(Some(FAKE_ID));
    // Il modello si valida con il catalogo dell'agente risolto.
    let err = call(&st, &h, INTAKE, "propose_tasks", json!({"tasks": [spec("x", &[])]})).unwrap_err();
    assert_eq!(
        err,
        format!("task 'x': invalid model for {FAKE_NAME}: {FOREIGN_MODEL} (see get_workspace_settings.agents)")
    );
    assert!(view(&st, &h).proposals.is_empty());

    queue(&st, &h, vec![fake_spec("a", &[])]);
    assert_eq!(agent_of(&st, &h, "a"), FAKE_ID);
    assert_eq!(status(&st, &h, "a"), "running");
    assert_eq!(launched(&st, &h, "a"), (FAKE_ID.to_string(), FAKE_MODEL_LARGE.to_string()));
}

#[test]
fn a_task_created_without_a_model_starts_with_the_workspace_model_of_the_fake_agent() {
    let _second = second();
    let (st, h) = host(Some(FAKE_ID));
    let err =
        boards::task_create(&st, &h, "ws-1", &json!({"id": "x", "title": "T", "task": "x", "model": FOREIGN_MODEL}))
            .unwrap_err();
    assert!(err.contains(&format!("invalid model for {FAKE_NAME}: {FOREIGN_MODEL}")), "{err}");

    set_workspace_model(&h, FAKE_ID, FAKE_MODEL_LARGE);
    create_bare(&st, &h, "a").unwrap();
    assert_eq!(agent_of(&st, &h, "a"), FAKE_ID);
    assert_eq!(launched(&st, &h, "a"), (FAKE_ID.to_string(), FAKE_MODEL_LARGE.to_string()));
}

#[test]
fn without_a_workspace_model_the_fake_agent_starts_with_its_catalog_default() {
    let _second = second();
    let (st, h) = host(Some(FAKE_ID));
    // Il modello del workspace per un altro agente non conta.
    set_workspace_model(&h, real_id(), REAL_WORKSPACE_MODEL);
    create_bare(&st, &h, "a").unwrap();
    assert_eq!(launched(&st, &h, "a"), (FAKE_ID.to_string(), FAKE_MODEL_SMALL.to_string()));
}

#[test]
fn the_worker_of_the_fake_agent_gets_its_env_staged_but_not_sent_to_the_frontend() {
    let _second = second();
    let (st, h) = host(Some(FAKE_ID));
    queue(&st, &h, vec![fake_spec("a", &[])]);
    let tab = session_of(&st, &h, "a");
    assert_eq!(h.staged_env(&tab), [(FAKE_ENV_NAME.to_string(), "accept-edits".to_string())]);
    let spawn = h.spawns().into_iter().find(|s| s.tab_id == tab).unwrap();
    assert!(!spawn.command.contains(FAKE_ENV_NAME));
}

#[test]
fn the_worker_of_claude_code_stages_no_env() {
    let _second = second();
    let (st, h) = host(Some(FAKE_ID));
    let mut real = fake_spec("r", &[]);
    real["agent"] = json!(real_id());
    real["bot"] = json!(h.bot_for_agent(real_id()));
    real["model"] = json!("opus");
    queue(&st, &h, vec![real]);
    assert_eq!(launched(&st, &h, "r").0, real_id());
    assert!(h.staged.lock().unwrap().is_empty());
}

#[test]
fn a_review_task_of_the_fake_agent_starts_its_reviewer_with_that_agent() {
    let _second = second();
    let (st, h) = host(None);
    let mut review = fake_spec("rev", &["a"]);
    review["kind"] = json!("review");
    review["agent"] = json!(FAKE_ID);
    review["bot"] = json!(h.bot_for_agent(FAKE_ID));
    queue(&st, &h, vec![spec("a", &[]), review]);
    assert_eq!((agent_of(&st, &h, "a"), agent_of(&st, &h, "rev")), (real_id().to_string(), FAKE_ID.to_string()));
    assert_eq!(launched(&st, &h, "a").0, real_id());

    finish(&st, &h, &session_of(&st, &h, "a"), "completed");
    assert_eq!(status(&st, &h, "rev"), "running");
    assert_eq!(launched(&st, &h, "rev"), (FAKE_ID.to_string(), FAKE_MODEL_LARGE.to_string()));
}

#[test]
fn the_fake_agent_has_its_interactive_launch_and_no_skill_entry() {
    let second = second();
    let launches = builtin_launch_list();
    let fake = launches.iter().find(|l| l.id == format!("builtin:{}", second.id())).expect("avvio dell'agente finto");
    assert_eq!((fake.agent_id.as_str(), fake.command.as_str()), (second.id(), fake.restore_command.as_str()));
    // Un avvio per agente abilitato e nient'altro: le skill partono con i Bot.
    let agents: Vec<String> = registry::enabled().iter().map(|a| format!("builtin:{}", a.id())).collect();
    assert_eq!(launches.iter().map(|l| l.id.clone()).collect::<Vec<_>>(), agents);
}

#[test]
fn get_workspace_settings_lists_both_integrations() {
    let _second = second();
    let (st, h) = host(Some(FAKE_ID));
    let out = call(&st, &h, INTAKE, "get_workspace_settings", json!({})).unwrap();
    let agents = out["agents"].as_array().unwrap();
    let ids: Vec<&str> = agents.iter().map(|a| a["id"].as_str().unwrap()).collect();
    let registered: Vec<&str> = registry::all().iter().map(|i| i.id()).collect();
    assert_eq!(ids, registered);
    assert_eq!(ids.last(), Some(&FAKE_ID));
    let fake = agents.last().unwrap();
    assert_eq!((fake["name"].as_str(), fake["is_default"].as_bool()), (Some(FAKE_NAME), Some(false)));
    let models: Vec<&str> = fake["models"].as_array().unwrap().iter().map(|m| m["id"].as_str().unwrap()).collect();
    assert_eq!(models, [FAKE_MODEL_SMALL, FAKE_MODEL_LARGE]);
    assert_eq!(fake["default_model"], FAKE_MODEL_SMALL);
    assert!(fake["rule_format"]["max_rules"].as_u64().is_some_and(|n| n > 0));
    assert_eq!(fake["readiness"], "ready");
    assert_eq!(agents[0]["is_default"], true);
    // Gli avvii ammessi comprendono l'agente finto e i Bot, nessuna vecchia voce dei preset.
    let allowed = out["allowed"]["default_launch"].as_array().unwrap();
    assert!(allowed.contains(&json!(format!("builtin:{FAKE_ID}"))) && allowed.contains(&json!(FAKE_BOT_LAUNCH)));
    assert!(!allowed.iter().any(|id| id.as_str().is_some_and(|id| id.starts_with("builtin:preset:"))));
}

#[test]
fn tasks_of_a_disabled_fake_agent_wait_with_the_reason_without_holding_back_the_others() {
    let second = second();
    let (st, h) = host(None);
    boards::board_set(&st, &h, "ws-1", None, Some(true)).unwrap();
    let mut fake_task = fake_spec("a", &[]);
    fake_task["agent"] = json!(FAKE_ID);
    fake_task["bot"] = json!(h.bot_for_agent(FAKE_ID));
    queue(&st, &h, vec![fake_task.clone(), spec("b", &[])]);

    second.disable();
    // Come dopo il cambio dalla pagina delle integrazioni.
    st.agents_changed(&h);
    boards::board_set(&st, &h, "ws-1", None, Some(false)).unwrap();
    // Stesso repository, consegna in place: se "a" tenesse il posto, "b" non partirebbe.
    assert_eq!((status(&st, &h, "a"), status(&st, &h, "b")), ("queued".into(), "running".into()));
    assert_eq!(task(&st, &h, "a")["agent_unavailable"], json!({"agent": FAKE_ID, "reason": "disabled"}));
    assert!(task(&st, &h, "b").get("agent_unavailable").is_none());
    assert_eq!(launched(&st, &h, "b").0, real_id());
    // Un nuovo task non puo' sceglierlo.
    fake_task["id"] = json!("c");
    let err = call(&st, &h, INTAKE, "propose_tasks", json!({"tasks": [fake_task]})).unwrap_err();
    assert_eq!(err, format!("task 'c': {FAKE_NAME} is disabled: enable it in the integrations settings"));
}

#[test]
fn a_task_for_a_real_second_agent_is_validated_with_its_catalog_and_gets_its_config_in_the_env() {
    const OTHER_ID: &str = "opencode";
    const OTHER_NAME: &str = "OpenCode";
    const OTHER_MODEL: &str = "opencode/gpt-5.5";
    const CONFIG_ENV: &str = "OPENCODE_CONFIG_CONTENT";
    let _second = second();
    let (st, h) = host(None);
    // Il modello di un altro agente non vale per questo.
    let mut foreign = spec("x", &[]);
    foreign["agent"] = json!(OTHER_ID);
    foreign["bot"] = json!(h.bot_for_agent(OTHER_ID));
    let err = call(&st, &h, INTAKE, "propose_tasks", json!({"tasks": [foreign]})).unwrap_err();
    assert!(err.contains(&format!("invalid model for {OTHER_NAME}: {FOREIGN_MODEL}")), "{err}");

    let mut task = spec("o", &[]);
    task["agent"] = json!(OTHER_ID);
    task["bot"] = json!(h.bot_for_agent(OTHER_ID));
    task["model"] = json!(OTHER_MODEL);
    queue(&st, &h, vec![task]);
    assert_eq!(agent_of(&st, &h, "o"), OTHER_ID);
    assert_eq!(status(&st, &h, "o"), "running");
    assert_eq!(launched(&st, &h, "o"), (OTHER_ID.to_string(), OTHER_MODEL.to_string()));

    // Modello e permessi viaggiano nell'ambiente del tab, mai nella riga di comando.
    let tab = session_of(&st, &h, "o");
    let env = h.staged_env(&tab);
    let (name, value) = &env[0];
    assert_eq!(name, CONFIG_ENV);
    let config: Value = serde_json::from_str(value).unwrap();
    assert_eq!(config["model"], OTHER_MODEL);
    assert_eq!(config["default_agent"], "build");
    let spawn = h.spawns().into_iter().find(|s| s.tab_id == tab).unwrap();
    assert!(!spawn.command.contains(CONFIG_ENV));
}

#[test]
fn a_plan_of_the_hub_takes_the_model_of_its_bot_on_the_workspace_agent() {
    let _second = second();
    let (st, h) = host(Some(FAKE_ID));
    boards::board_set(&st, &h, "ws-1", None, Some(true)).unwrap();
    // Il Bot `intake` chiede la fascia forte: quella dell'agente del workspace.
    let id = planner::create_hub_plan_task(&st, &h, "ws-1", "Plan dark theme", "Add a dark theme", "intake").unwrap();
    let t = task(&st, &h, &id);
    assert_eq!((t["kind"].as_str(), t["model"].as_str()), (Some("plan"), Some(FAKE_MODEL_LARGE)));
    assert_eq!(agent_of(&st, &h, &id), FAKE_ID);
    // Il Bot `general` lascia il modello predefinito dell'agente.
    let general = planner::create_hub_plan_task(&st, &h, "ws-1", "Plan theme", "Add a theme", "general").unwrap();
    assert!(task(&st, &h, &general)["model"].is_null());
    assert_eq!(agent_of(&st, &h, &general), FAKE_ID);
}
