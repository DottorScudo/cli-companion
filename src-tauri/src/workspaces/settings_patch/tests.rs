use super::super::{MAX_LAUNCH_NAME_LEN, MAX_REPO_COMMANDS, MAX_REPO_COMMAND_LEN, MAX_RULES_LEN};
use super::*;
use crate::agents::{registry, worker_rule_format, PermissionMode, RuleFormat};
use crate::validate::ErrorCode;
use serde_json::json;

mod agents;

/// Id dell'agente predefinito: quello dei worker dei workspace di prova.
fn agent_id() -> &'static str {
    registry::default_agent().id()
}

/// Limiti delle regole dell'agente predefinito dei worker.
fn rule_limits() -> RuleFormat {
    worker_rule_format(agent_id()).expect("the default worker agent applies permission rules")
}

/// La voce del solo Bot abilitato nei test.
const TEST_BOT_LAUNCH: &str = "bot:intake";

/// Gli avvii integrati e il Bot dei test: quelli che `default_launch` puo' nominare.
fn shared() -> Vec<String> {
    shared_launch_ids(vec![TEST_BOT_LAUNCH.to_string()])
}

/// Proposta che cambia una sola voce di `worker_agents` dell'agente predefinito.
fn agent_patch(fields: serde_json::Value) -> serde_json::Value {
    json!({"worker_agents": {agent_id(): fields}})
}

fn ws() -> Workspace {
    let raw = r#"{"id":"ws-1","name":"p","kind":"windows","path":"C:\\p","created_at":1}"#;
    serde_json::from_str(raw).unwrap()
}

fn launch(id: &str) -> Launch {
    Launch { id: id.into(), name: format!("name {id}"), command: format!("cmd {id}") }
}

fn patch(v: serde_json::Value) -> SettingsPatch {
    SettingsPatch::parse(&v).unwrap()
}

fn id_gen() -> impl FnMut() -> String {
    let mut n = 0;
    move || {
        n += 1;
        format!("new-{n}")
    }
}

fn apply(p: &SettingsPatch, w: &mut Workspace) -> Result<(), ValidationError> {
    p.apply(w, &shared(), &mut id_gen())
}

#[test]
fn parse_distinguishes_absent_from_null() {
    let p = patch(json!({"agent": null, "default_launch": null, "repos": {"a": null, "b": {"base_branch": null}}}));
    assert_eq!(p.agent, Some(None));
    assert_eq!(p.default_launch, Some(None));
    assert_eq!(p.worker_rules, None);
    let repos = p.repos.unwrap();
    assert_eq!(repos["a"], None);
    assert_eq!(repos["b"].as_ref().unwrap().base_branch, Some(None));
    assert_eq!(repos["b"].as_ref().unwrap().container_root, None);
    let p = patch(json!({"agent": "x", "worker_agents": {"x": {"model": null, "allow": []}}}));
    assert_eq!(p.agent, Some(Some("x".to_string())));
    let entry = &p.worker_agents.unwrap()["x"];
    assert_eq!((entry.model.clone(), entry.allow.clone(), entry.deny.clone()), (Some(None), Some(vec![]), None));
}

#[test]
fn parse_roundtrips_absent_and_null() {
    let v = json!({
        "agent": null,
        "worker_agents": {"x": {"model": null, "deny": ["a"]}},
        "repos": {"a": null, "b": {"base_branch": null}}
    });
    assert_eq!(serde_json::to_value(patch(v.clone())).unwrap(), v);
}

#[test]
fn parse_empty_object_is_empty_patch() {
    assert!(patch(json!({})).is_empty());
    assert!(!patch(json!({"use_schedule": false})).is_empty());
}

#[test]
fn parse_rejects_unknown_fields_and_non_objects() {
    let e = SettingsPatch::parse(&json!({"theme": "dark"})).unwrap_err();
    assert!(e.starts_with("invalid settings: ") && e.contains("theme"), "{e}");
    assert!(SettingsPatch::parse(&json!({"repos": {"a": {"nope": 1}}})).is_err());
    assert!(SettingsPatch::parse(&json!({"worker_agents": {"a": {"nope": 1}}})).is_err());
    for legacy in ["worker_model", "worker_allow", "worker_deny"] {
        assert!(SettingsPatch::parse(&json!({ legacy: null })).is_err(), "{legacy}");
    }
    assert!(SettingsPatch::parse(&json!({"delivery": {"nope": 1}})).is_err());
    assert!(SettingsPatch::parse(&json!({"launches": [{"name": "a", "command": "b", "x": 1}]})).is_err());
    for v in [json!([]), json!("x"), json!(null), json!(3)] {
        let e = SettingsPatch::parse(&v).unwrap_err();
        assert!(e.contains("expected a JSON object"), "{e}");
    }
}

#[test]
fn parse_rejects_fixed_fields_with_clear_message() {
    for field in ["id", "name", "path", "kind", "distro", "shell", "archived", "created_at"] {
        let e = SettingsPatch::parse(&json!({ field: "x", "use_schedule": true })).unwrap_err();
        assert!(e.starts_with(&format!("`{field}` cannot be changed by a proposal")), "{e}");
        assert!(e.contains("fixed: id, name, path, kind, distro, shell, archived, created_at"), "{e}");
    }
}

#[test]
fn normalized_cleans_values() {
    let mut w = ws();
    w.launches = vec![launch("l1")];
    let p = patch(json!({
        "worker_rules": "  no tests  ",
        "agent": format!(" {} ", agent_id()),
        "worker_permission": " accept-edits ",
        "worker_agents": agent_patch(json!({
            "model": "  ",
            "allow": [" Bash(git:*) ", "", "Bash(git:*)", "Read"],
            "deny": ["WebFetch"]
        }))["worker_agents"],
        "delivery": {"mode": "commit", "parallel": true},
        "use_schedule": true,
        "default_launch": " l1 ",
        "launches": [{"id": "l1", "name": " Dev ", "command": " ls "}],
        "repos": {"api/": {"base_branch": " main ", "setup": [" a ", ""]}}
    }));
    let n = p.normalized(&w, &shared()).unwrap();
    assert_eq!(n.worker_rules.as_deref(), Some("no tests"));
    assert_eq!(n.agent, Some(Some(agent_id().to_string())));
    assert_eq!(n.worker_permission.as_deref(), Some("accept-edits"));
    let entry = &n.worker_agents.unwrap()[agent_id()];
    assert_eq!(entry.model, Some(None));
    assert_eq!(entry.allow, Some(vec!["Bash(git:*)".to_string(), "Read".to_string()]));
    assert_eq!(entry.deny, Some(vec!["WebFetch".to_string()]));
    // `parallel` non ha senso senza branch: scartato.
    assert_eq!(
        n.delivery,
        Some(DeliveryPatch { mode: Some(DeliveryMode::Commit), parallel: Some(false), merge_method: None })
    );
    assert_eq!(n.use_schedule, Some(true));
    assert_eq!(n.default_launch, Some(Some("l1".to_string())));
    assert_eq!(n.launches, Some(vec![LaunchPatch { id: Some("l1".into()), name: "Dev".into(), command: "ls".into() }]));
    let api = n.repos.unwrap()["api"].clone().unwrap();
    assert_eq!(api.base_branch, Some(Some("main".to_string())));
    assert_eq!(api.setup, Some(vec!["a".to_string()]));
    assert_eq!(api.cleanup, None);
}

#[test]
fn delivery_keeps_parallel_with_branches_and_merges_onto_current() {
    let mut w = ws();
    w.delivery = Delivery { mode: DeliveryMode::Pr, parallel: true, merge_method: MergeMethod::Merge };
    let n = patch(json!({"delivery": {"merge_method": "rebase"}})).normalized(&w, &shared()).unwrap();
    assert_eq!(n.delivery, Some(DeliveryPatch { merge_method: Some(MergeMethod::Rebase), ..Default::default() }));
    // Cambiando solo la modalita', `parallel` esistente viene riletto dalla normalizzazione.
    let n = patch(json!({"delivery": {"mode": "push"}})).normalized(&w, &shared()).unwrap();
    assert_eq!(n.delivery, Some(DeliveryPatch { mode: Some(DeliveryMode::Push), ..Default::default() }));
    let mut next = w.clone();
    patch(json!({"delivery": {"mode": "push"}})).apply(&mut next, &shared(), &mut id_gen()).unwrap();
    assert!(!next.delivery.parallel);
}

#[test]
fn invalid_values_fail_with_the_english_texts_for_the_model() {
    let mut w = ws();
    w.launches = vec![launch("l1")];
    let too_many_launches: Vec<_> =
        (0..=MAX_PATCH_LAUNCHES).map(|i| json!({"name": format!("n{i}"), "command": "c"})).collect();
    let cases = [
        (json!({"worker_rules": "x".repeat(MAX_RULES_LEN + 1)}), "worker_rules"),
        (json!({"worker_rules": "a\u{1}b"}), "worker_rules"),
        (agent_patch(json!({"model": "no good!"})), "model not accepted by the agent: no good!"),
        (json!({"worker_permission": "root"}), "worker_permission"),
        (agent_patch(json!({"allow": ["bad rule"]})), "worker_allow: rule \"bad rule\""),
        (agent_patch(json!({"allow": ["a\nb"]})), "worker_allow: rule"),
        (
            agent_patch(json!({"deny": ["mcp__srv__tool(x)"]})),
            "worker_deny: rule \"mcp__srv__tool(x)\": this kind of rule takes no arguments",
        ),
        (
            agent_patch(json!({"deny": [format!("Bash({})", "a".repeat(rule_limits().max_rule_chars))]})),
            "worker_deny: rule",
        ),
        (
            agent_patch(json!({"allow": (0..=rule_limits().max_rules).map(|i| format!("R{i}")).collect::<Vec<_>>()})),
            "worker_allow: too many rules",
        ),
        (json!({"delivery": {}}), "delivery"),
        (json!({"launches": [{"name": "", "command": "c"}]}), "launches[0]: launch name is missing"),
        (
            json!({"launches": [{"name": "n".repeat(MAX_LAUNCH_NAME_LEN + 1), "command": "c"}]}),
            "launches[0]: launch name too long",
        ),
        (
            json!({"launches": [{"name": "n", "command": "c"}, {"name": "n", "command": " "}]}),
            "launches[1]: launch command is missing",
        ),
        (
            json!({"launches": [{"id": "ghost", "name": "n", "command": "c"}]}),
            "launches[0]: launch not found: an id must be",
        ),
        (
            json!({"launches": [{"id": "l1", "name": "n", "command": "c"}, {"id": "l1", "name": "m", "command": "c"}]}),
            "launches[1]: duplicate launch id",
        ),
        (json!({"launches": too_many_launches}), "too many launches"),
        (json!({"default_launch": "nope"}), "invalid default_launch"),
        (json!({"repos": {"x": {"base_branch": "no good"}}}), "repos['x']: invalid base_branch: no good"),
        (json!({"repos": {"x": {"container_root": "a\nb"}}}), "repos['x']: invalid container_root"),
        (json!({"repos": {"x": {"copy_ignored": ["/abs"]}}}), "repos['x']: copy_ignored paths must be relative"),
        (json!({"repos": {"x": {"copy_ignored": [".."]}}}), "repos['x']: invalid copy_ignored path"),
        (json!({"repos": {"x": {"setup": ["a\nb"]}}}), "repos['x']: setup commands must be single-line"),
        (json!({"repos": {"x": {"cleanup": ["a\nb"]}}}), "repos['x']: cleanup commands must be single-line"),
        (
            json!({"repos": {"x": {"setup": ["y".repeat(MAX_REPO_COMMAND_LEN + 1)]}}}),
            "repos['x']: setup command too long",
        ),
        (
            json!({"repos": {"x": {"cleanup": vec!["y"; MAX_REPO_COMMANDS + 1]}}}),
            "repos['x']: too many cleanup commands",
        ),
        (
            json!({"repos": {"x": {"copy_ignored": (0..=MAX_REPO_COMMANDS).map(|i| format!("p{i}")).collect::<Vec<_>>()}}}),
            "repos['x']: too many copy_ignored paths",
        ),
        (json!({"repos": {"x": {}}}), "repos['x']: has no fields"),
        (json!({"repos": {"x": {"remove_force": true}, "x/": {"remove_force": true}}}), "duplicate repo key"),
        (json!({"repos": {"/abs": {"remove_force": true}}}), "repos['/abs']"),
    ];
    for (v, expect) in cases {
        let err = patch(v.clone()).normalized(&w, &shared()).unwrap_err();
        let en = err.message_en();
        assert!(en.contains(expect), "{v}: {en} (it: {})", err.message_it());
    }
}

#[test]
fn every_rejection_has_a_distinct_italian_and_english_text() {
    let w = ws();
    for v in [
        json!({"worker_rules": "a\u{1}b"}),
        json!({"worker_permission": "root"}),
        agent_patch(json!({"allow": ["bad rule"]})),
        agent_patch(json!({})),
        json!({"agent": "nope"}),
        json!({"launches": [{"name": "", "command": "c"}]}),
        json!({"repos": {"x": {"base_branch": "no good"}}}),
        json!({"repos": {"x": {}}}),
        json!({"delivery": {}}),
    ] {
        let err = patch(v.clone()).normalized(&w, &shared()).unwrap_err();
        assert_ne!(err.message_it(), err.message_en(), "{v}");
        assert_eq!(String::from(err.clone()), err.message_it(), "i comandi dell'interfaccia usano l'italiano");
    }
}

#[test]
fn errors_carry_a_code_and_the_place_where_they_happened() {
    let mut w = ws();
    w.launches = vec![launch("l1")];
    let code = |v: serde_json::Value| patch(v).normalized(&w, &shared()).unwrap_err().code();
    assert_eq!(code(json!({"worker_rules": "a\u{1}b"})), ErrorCode::RulesControlChars);
    assert_eq!(code(json!({"worker_permission": "root"})), ErrorCode::PermissionInvalid);
    assert_eq!(code(json!({"default_launch": "nope"})), ErrorCode::DefaultLaunchInvalid);
    assert_eq!(code(json!({"delivery": {}})), ErrorCode::DeliveryEmpty);
    assert_eq!(code(json!({"repos": {"x": {}}})), ErrorCode::RepoPatchEmpty);
    assert_eq!(
        code(json!({"repos": {"x": {"remove_force": true}, "x/": {"remove_force": true}}})),
        ErrorCode::RepoKeyDuplicate
    );
    assert_eq!(code(json!({"launches": [{"id": "ghost", "name": "n", "command": "c"}]})), ErrorCode::LaunchIdUnknown);
    let err = patch(json!({"launches": [{"name": "n", "command": "c"}, {"name": "", "command": "c"}]}))
        .normalized(&w, &shared())
        .unwrap_err();
    assert_eq!(err.message_it(), "launches[1]: nome avvio mancante");
    let err = patch(json!({"repos": {"api/": {"setup": ["a\nb"]}}})).normalized(&w, &shared()).unwrap_err();
    assert_eq!(err.message_it(), "repos['api']: il comando di setup deve stare su una riga: a\nb");
}

#[test]
fn rule_errors_name_their_list_and_keep_limits() {
    let w = ws();
    let too_long = format!("Bash({})", "a".repeat(rule_limits().max_rule_chars));
    let prefix = format!("worker_agents['{}']: ", agent_id());
    let err = patch(agent_patch(json!({"deny": [too_long]}))).normalized(&w, &shared()).unwrap_err();
    assert_eq!(err.code(), ErrorCode::PermissionRuleTooLong);
    assert!(err.message_en().starts_with(&format!("{prefix}worker_deny: rule \"Bash(aaa")), "{}", err.message_en());
    let max_rule_chars = rule_limits().max_rule_chars;
    assert!(err.message_en().ends_with(&format!("longer than {max_rule_chars} characters")));
    assert!(err.message_it().starts_with(&format!("{prefix}Vietati: regola \"Bash(aaa")), "{}", err.message_it());
    let err = patch(agent_patch(json!({"allow": ["a b"]}))).normalized(&w, &shared()).unwrap_err();
    assert_eq!(err.message_en(), format!("{prefix}worker_allow: rule \"a b\": not a valid rule for the worker agent"));
    assert_eq!(
        err.message_it(),
        format!("{prefix}Consentiti: regola \"a b\": forma non valida per l'agente dei worker")
    );
    let many: Vec<String> = (0..=rule_limits().max_rules).map(|i| format!("R{i}")).collect();
    let err = patch(agent_patch(json!({"deny": many}))).normalized(&w, &shared()).unwrap_err();
    let max_rules = rule_limits().max_rules;
    assert_eq!(err.message_en(), format!("{prefix}worker_deny: too many rules (max {max_rules})"));
}

#[test]
fn default_launch_is_checked_against_launches_after_the_patch() {
    let mut w = ws();
    w.launches = vec![launch("l1"), launch("l2")];
    // l2 viene tolto dalla lista proposta: non puo' essere il predefinito.
    let both = patch(json!({"launches": [{"id": "l1", "name": "a", "command": "b"}], "default_launch": "l2"}));
    assert!(both.normalized(&w, &shared()).is_err());
    let ok = patch(json!({"launches": [{"id": "l1", "name": "a", "command": "b"}], "default_launch": "l1"}));
    assert!(ok.normalized(&w, &shared()).is_ok());
    // "shell" e gli integrati sono sempre ammessi.
    assert!(patch(json!({"default_launch": "shell"})).normalized(&w, &shared()).is_ok());
    assert!(patch(json!({"default_launch": "builtin:claude-code"})).normalized(&w, &shared()).is_ok());
}

#[test]
fn default_launch_accepts_an_enabled_bot_only() {
    let w = ws();
    let n = patch(json!({"default_launch": TEST_BOT_LAUNCH})).normalized(&w, &shared()).unwrap();
    assert_eq!(n.default_launch, Some(Some(TEST_BOT_LAUNCH.to_string())));
    let err = patch(json!({"default_launch": "bot:gone"})).normalized(&w, &shared()).unwrap_err();
    assert_eq!(err.code(), ErrorCode::DefaultLaunchInvalid);
}

#[test]
fn an_old_preset_launch_in_a_saved_proposal_goes_to_its_bot() {
    let w = ws();
    let launch = |id: &str, shared: &[String]| {
        patch(json!({"default_launch": id}))
            .normalized(&w, shared)
            .map(|n| n.default_launch.flatten())
            .map_err(|e| e.code())
    };
    let derived = shared_launch_ids(vec![TEST_BOT_LAUNCH.into(), "bot:intake-x".into()]);
    assert_eq!(launch("builtin:preset:task-intake", &shared()), Ok(Some(TEST_BOT_LAUNCH.into())));
    assert_eq!(launch("builtin:preset:task-intake@x", &derived), Ok(Some("bot:intake-x".into())));
    // Senza il Bot derivato si ripiega sul predefinito, non sulla shell.
    assert_eq!(launch("builtin:preset:task-intake@x", &shared()), Ok(Some(TEST_BOT_LAUNCH.into())));
    // Il Bot del preset non e' abilitato: la proposta resta non valida.
    assert_eq!(launch("builtin:preset:project-setup", &shared()), Err(ErrorCode::DefaultLaunchInvalid));
}

#[test]
fn apply_is_all_or_nothing() {
    let mut w = ws();
    w.launches = vec![launch("l1")];
    let before = w.clone();
    let p = patch(json!({
        "worker_rules": "valid",
        "use_schedule": true,
        "repos": {"a": {"setup": ["ok"]}, "b": {"base_branch": "no good"}}
    }));
    assert!(apply(&p, &mut w).is_err());
    assert_eq!(w, before);
}

#[test]
fn apply_changes_everything_at_once() {
    let mut w = ws();
    w.launches = vec![launch("l1"), launch("l2")];
    w.default_launch = Some("l2".into());
    w.repos.insert("api".into(), RepoConfig { base_branch: Some("main".into()), ..Default::default() });
    w.repos.insert("old".into(), RepoConfig::default());
    let p = patch(json!({
        "worker_rules": "rules",
        "agent": agent_id(),
        "worker_permission": "plan",
        "worker_agents": agent_patch(json!({"model": "opus", "allow": ["Read"], "deny": ["WebFetch"]}))["worker_agents"],
        "delivery": {"mode": "local_merge", "parallel": true},
        "use_schedule": true,
        "launches": [{"name": "fresh", "command": "c"}, {"id": "l1", "name": "renamed", "command": "x"}],
        "repos": {"api": {"base_branch": null, "setup": ["s"]}, "old": null, "new": {"remove_force": true}}
    }));
    apply(&p, &mut w).unwrap();
    assert_eq!(w.worker.rules, "rules");
    assert_eq!(w.agent.as_deref(), Some(agent_id()));
    assert_eq!(w.worker.model_for(agent_id()), Some("opus"));
    assert_eq!(w.worker.permission, PermissionMode::Plan);
    assert_eq!(w.worker.rules_for(agent_id()), (&["Read".to_string()][..], &["WebFetch".to_string()][..]));
    assert_eq!(
        w.delivery,
        Delivery { mode: DeliveryMode::LocalMerge, parallel: true, merge_method: MergeMethod::Squash }
    );
    assert!(w.use_schedule);
    // Nuovo avvio con id generato, l1 mantiene l'id, l2 e' sparito (era il predefinito).
    let ids: Vec<_> = w.launches.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(ids, ["new-1", "l1"]);
    assert_eq!(w.launches[1].name, "renamed");
    assert_eq!(w.default_launch, None);
    assert_eq!(w.repos["api"].base_branch, None);
    assert_eq!(w.repos["api"].setup, ["s"]);
    assert!(!w.repos.contains_key("old"));
    assert!(w.repos["new"].remove_force);
}

#[test]
fn apply_default_launch_variants() {
    let mut w = ws();
    w.launches = vec![launch("l1"), launch("l2")];
    w.default_launch = Some("l1".into());
    // Il predefinito resta se l'avvio resta.
    apply(&patch(json!({"launches": [{"id": "l1", "name": "a", "command": "b"}]})), &mut w).unwrap();
    assert_eq!(w.default_launch.as_deref(), Some("l1"));
    // Un predefinito integrato non viene toccato dalla sostituzione della lista.
    w.default_launch = Some("shell".into());
    apply(&patch(json!({"launches": []})), &mut w).unwrap();
    assert!(w.launches.is_empty());
    assert_eq!(w.default_launch.as_deref(), Some("shell"));
    // Impostazione esplicita e ritorno all'ereditarieta'.
    w.launches = vec![launch("l1")];
    apply(&patch(json!({"default_launch": "l1"})), &mut w).unwrap();
    assert_eq!(w.default_launch.as_deref(), Some("l1"));
    apply(&patch(json!({"default_launch": null})), &mut w).unwrap();
    assert_eq!(w.default_launch, None);
}

#[test]
fn apply_absent_fields_change_nothing() {
    let mut w = ws();
    w.worker.rules = "keep".into();
    w.launches = vec![launch("l1")];
    w.default_launch = Some("l1".into());
    w.repos.insert("api".into(), RepoConfig { setup: vec!["s".into()], ..Default::default() });
    let before = w.clone();
    apply(&patch(json!({})), &mut w).unwrap();
    assert_eq!(w, before);
    apply(&patch(json!({"repos": {"api": {"remove_force": true}}})), &mut w).unwrap();
    assert_eq!(w.repos["api"].setup, ["s"]);
    assert!(w.repos["api"].remove_force);
    assert_eq!(w.worker.rules, "keep");
}

#[test]
fn apply_repo_null_values_clear_or_remove() {
    let mut w = ws();
    w.repos.insert(
        ".".into(),
        RepoConfig { base_branch: Some("dev".into()), container_root: Some("/srv".into()), ..Default::default() },
    );
    apply(&patch(json!({"repos": {".": {"container_root": null}}})), &mut w).unwrap();
    assert_eq!((w.repos["."].base_branch.as_deref(), w.repos["."].container_root.as_deref()), (Some("dev"), None));
    apply(&patch(json!({"repos": {"./": null}})), &mut w).unwrap();
    assert!(w.repos.is_empty());
    // Rimuovere un repository senza configurazione e' un no-op.
    apply(&patch(json!({"repos": {"ghost": null}})), &mut w).unwrap();
}

#[test]
fn current_reports_only_touched_fields() {
    let mut w = ws();
    w.worker.rules = "old".into();
    w.agent = Some(agent_id().into());
    w.worker.set_agent(
        agent_id(),
        AgentWorkerSettings { model: Some("sonnet".into()), allow: vec!["Read".into()], deny: Vec::new() },
    );
    w.launches = vec![launch("l1")];
    w.default_launch = Some("l1".into());
    w.delivery = Delivery { mode: DeliveryMode::Pr, parallel: true, merge_method: MergeMethod::Merge };
    w.repos.insert(
        "api".into(),
        RepoConfig { base_branch: Some("main".into()), setup: vec!["a".into()], ..Default::default() },
    );
    w.repos.insert("gone".into(), RepoConfig { cleanup: vec!["c".into()], remove_force: true, ..Default::default() });
    let p = patch(json!({
        "worker_rules": "new",
        "agent": null,
        "worker_agents": agent_patch(json!({"model": null, "allow": ["Bash"]}))["worker_agents"],
        "delivery": {"mode": "commit"},
        "default_launch": null,
        "launches": [],
        "repos": {"api": {"setup": ["b"]}, "fresh": {"setup": ["x"]}, "gone": null}
    }))
    .normalized(&w, &shared())
    .unwrap();
    let cur = p.current(&w);
    assert_eq!(cur.worker_rules.as_deref(), Some("old"));
    assert_eq!(cur.agent, Some(Some(agent_id().to_string())));
    let entry = &cur.worker_agents.unwrap()[agent_id()];
    assert_eq!(
        (entry.model.clone(), entry.allow.clone(), entry.deny.clone()),
        (Some(Some("sonnet".to_string())), Some(vec!["Read".to_string()]), None)
    );
    assert_eq!(cur.delivery, Some(DeliveryPatch { mode: Some(DeliveryMode::Pr), ..Default::default() }));
    assert_eq!(cur.default_launch, Some(Some("l1".to_string())));
    assert_eq!(
        cur.launches,
        Some(vec![LaunchPatch { id: Some("l1".into()), name: "name l1".into(), command: "cmd l1".into() }])
    );
    // Campi non toccati: assenti.
    assert!(cur.worker_permission.is_none() && cur.use_schedule.is_none());
    let repos = cur.repos.unwrap();
    assert_eq!(repos["api"], Some(RepoPatch { setup: Some(vec!["a".to_string()]), ..Default::default() }));
    assert_eq!(repos["fresh"], None);
    let gone = repos["gone"].clone().unwrap();
    assert_eq!(gone.cleanup, Some(vec!["c".to_string()]));
    assert_eq!(gone.remove_force, Some(true));
    assert_eq!(gone.base_branch, Some(None));
    assert!(gone.copy_ignored.is_some() && gone.setup.is_some() && gone.container_root.is_some());
    // Una patch di un solo campo non riporta altro.
    assert_eq!(
        patch(json!({"use_schedule": true})).current(&w),
        SettingsPatch { use_schedule: Some(false), ..Default::default() }
    );
}

#[test]
fn sensitive_lists_commands_and_permissions_in_order() {
    let all = patch(json!({
        "worker_rules": "r",
        "agent": "x",
        "worker_permission": "bypass",
        "worker_agents": {"b": {"deny": []}, "a": {"model": "m", "allow": [], "deny": []}, "c": {"model": null}},
        "delivery": {"mode": "push"},
        "use_schedule": true,
        "default_launch": null,
        "launches": [],
        "repos": {"api/web": {"cleanup": ["c"]}, ".": {"setup": ["s"], "cleanup": ["c"], "remove_force": true}, "gone": null, "plain": {"base_branch": "x"}}
    }));
    assert_eq!(
        all.sensitive(),
        [
            "worker_permission",
            "worker_agents[a].allow",
            "worker_agents[a].deny",
            "worker_agents[b].deny",
            "launches",
            "default_launch",
            "repos[.].setup",
            "repos[.].cleanup",
            "repos[api/web].cleanup",
        ]
    );
}

#[test]
fn sensitive_is_empty_for_harmless_patches() {
    let p = patch(json!({
        "worker_rules": "r",
        "agent": null,
        "worker_agents": {"a": {"model": "m"}},
        "delivery": {"parallel": true},
        "use_schedule": false,
        "repos": {"a": null, "b": {"base_branch": "x", "container_root": "/srv", "copy_ignored": [".env"], "remove_force": true}}
    }));
    assert!(p.sensitive().is_empty());
    assert!(patch(json!({})).sensitive().is_empty());
}

#[test]
fn needs_reconcile_and_repo_keys() {
    assert!(patch(json!({"delivery": {"mode": "push"}})).needs_reconcile());
    assert!(patch(json!({"use_schedule": false})).needs_reconcile());
    assert!(!patch(json!({"worker_rules": "x", "repos": {"a": null}})).needs_reconcile());
    let w = ws();
    let n = patch(json!({"repos": {"b/": {"remove_force": true}, "./": null, "a": null}}))
        .normalized(&w, &shared())
        .unwrap();
    assert_eq!(n.repo_keys(), [".", "a", "b"]);
    assert!(patch(json!({})).repo_keys().is_empty());
}

#[test]
fn e2e_is_set_kept_and_removed_by_a_patch() {
    let mut w = ws();
    let set = json!({"repos": {"api": {"e2e": {"command": " npm run e2e ", "junit": ".\\out\\junit.xml"}}}});
    apply(&patch(set), &mut w).unwrap();
    let e2e = w.repos["api"].e2e.clone().unwrap();
    assert_eq!((e2e.command.as_str(), e2e.junit.as_str(), e2e.timeout_min), ("npm run e2e", "out/junit.xml", 20));
    // Assente: resta; altri campi non lo toccano.
    apply(&patch(json!({"repos": {"api": {"remove_force": true}}})), &mut w).unwrap();
    assert_eq!(w.repos["api"].e2e.as_ref(), Some(&e2e));
    // null: lo rimuove.
    apply(&patch(json!({"repos": {"api": {"e2e": null}}})), &mut w).unwrap();
    assert!(w.repos["api"].e2e.is_none());
    // Invalido: nulla cambia.
    let before = w.clone();
    let bad = json!({"repos": {"api": {"e2e": {"command": "x", "junit": "../j.xml"}}}});
    assert!(apply(&patch(bad), &mut w).is_err());
    assert_eq!(w, before);
    assert!(
        SettingsPatch::parse(&json!({"repos": {"api": {"e2e": {"command": "x", "junit": "j", "cwd": "."}}}})).is_err()
    );
    assert!(SettingsPatch::parse(&json!({"repos": {"api": {"e2e": {"command": "x"}}}})).is_err());
}

#[test]
fn e2e_current_and_sensitive() {
    let mut w = ws();
    let cfg = crate::workspaces::E2eConfig { command: "c".into(), junit: "j.xml".into(), timeout_min: 5 };
    w.repos.insert("api".into(), RepoConfig { e2e: Some(cfg.clone()), ..Default::default() });
    let set = patch(json!({"repos": {"api": {"e2e": {"command": "d", "junit": "j.xml"}}, "web": {"e2e": null}}}));
    assert_eq!(set.sensitive(), ["repos[api].e2e"], "a removal is not sensitive");
    let cur = set.current(&w);
    assert_eq!(cur.repos.as_ref().unwrap()["api"].as_ref().unwrap().e2e, Some(Some(cfg.clone())));
    assert_eq!(cur.repos.as_ref().unwrap()["web"], None);
    // Rimuovere un repository mostra l'e2e attuale tra i valori persi.
    let gone = patch(json!({"repos": {"api": null}})).current(&w);
    assert_eq!(gone.repos.unwrap()["api"].as_ref().unwrap().e2e, Some(Some(cfg)));
}

#[test]
fn e2e_errors_have_english_texts() {
    let w = ws();
    let cases = [
        (json!({"command": "  ", "junit": "j"}), "repos['x']: e2e command is missing"),
        (json!({"command": "a\nb", "junit": "j"}), "repos['x']: e2e command must be single-line"),
        (json!({"command": "y".repeat(MAX_REPO_COMMAND_LEN + 1), "junit": "j"}), "repos['x']: e2e command too long"),
        (json!({"command": "c", "junit": ""}), "repos['x']: e2e junit path is missing"),
        (json!({"command": "c", "junit": "/abs"}), "repos['x']: e2e junit path must be relative"),
        (json!({"command": "c", "junit": "../j"}), "repos['x']: invalid e2e junit path (no '..')"),
        (json!({"command": "c", "junit": "j", "timeout_min": 0}), "repos['x']: invalid e2e timeout_min"),
        (json!({"command": "c", "junit": "j", "timeout_min": 61}), "repos['x']: invalid e2e timeout_min"),
    ];
    for (e2e, expect) in cases {
        let err = patch(json!({"repos": {"x": {"e2e": e2e}}})).normalized(&w, &shared()).unwrap_err();
        let en = err.message_en();
        assert!(en.contains(expect), "{e2e}: {en} (it: {})", err.message_it());
        assert_ne!(en, err.message_it());
    }
}
