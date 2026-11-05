use super::*;
use crate::connectors::commands::support::{contact_file, fixture, source_file, FakeWorld};
use crate::connectors::limits::MAX_ROUTES_PER_INSTANCE;
use crate::connectors::model::{RouteThread, RouteWake};
use crate::connectors::testing::{link, route, INSTANCE};
use serde_json::json;

fn input() -> RouteInput {
    RouteInput {
        label: "Posta".to_string(),
        filter: json!({"query": "da:capo"}),
        channel: "general".to_string(),
        thread: RouteThread::NewPerItem,
        wake: "none".to_string(),
        enabled: true,
    }
}

fn existing(thread_id: &str) -> RouteInput {
    RouteInput { thread: RouteThread::Existing { thread_id: thread_id.to_string() }, ..input() }
}

fn world() -> FakeWorld {
    FakeWorld {
        workspaces: vec!["ws-1".to_string()],
        threads: vec![("general".to_string(), "thr-1".to_string())],
        hub_off: false,
    }
}

#[test]
fn a_route_is_created_for_a_source() {
    let f = fixture(source_file());
    let created = create_route(&f.rt, &world(), INSTANCE, input()).unwrap();
    assert!(crate::connectors::ids::valid_route_id(&created.id));
    assert_eq!((created.instance.as_str(), created.created_at), (INSTANCE, 1_700_000_000_000));
    assert_eq!(f.rt.snapshot().routes, vec![created]);
}

#[test]
fn only_sources_known_instances_and_valid_inputs_get_routes() {
    let f = fixture(contact_file());
    assert!(create_route(&f.rt, &world(), INSTANCE, input()).unwrap_err().contains("fonte"));
    let f = fixture(source_file());
    assert!(create_route(&f.rt, &world(), "con-9-9", input()).is_err());
    assert!(create_route(&f.rt, &world(), "non valido", input()).is_err());
    let mut bad = input();
    bad.filter = json!({});
    assert!(create_route(&f.rt, &world(), INSTANCE, bad).is_err());
    let mut wake = input();
    wake.wake = "owner".to_string();
    assert!(create_route(&f.rt, &world(), INSTANCE, wake).is_ok(), "la fonte finta puo' svegliare");
    assert_eq!(f.rt.snapshot().routes.len(), 1, "nulla salvato dai rifiuti");
}

#[test]
fn the_channel_must_exist() {
    let f = fixture(source_file());
    let mut ws = input();
    ws.channel = "ws-1".to_string();
    assert!(create_route(&f.rt, &world(), INSTANCE, ws).is_ok());
    let mut gone = input();
    gone.channel = "ws-sparito".to_string();
    let err = create_route(&f.rt, &world(), INSTANCE, gone).unwrap_err();
    assert!(err.contains("canale"), "{err}");
}

#[test]
fn an_existing_thread_must_exist_in_the_hub() {
    let f = fixture(source_file());
    assert!(create_route(&f.rt, &world(), INSTANCE, existing("thr-1")).is_ok());
    let err = create_route(&f.rt, &world(), INSTANCE, existing("thr-2")).unwrap_err();
    assert!(err.contains("thread"), "{err}");
    let off = FakeWorld { hub_off: true, ..world() };
    let err = create_route(&f.rt, &off, INSTANCE, existing("thr-1")).unwrap_err();
    assert!(err.contains("spento"), "l'HUB spento non e' 'il thread non esiste': {err}");
    assert!(create_route(&f.rt, &off, INSTANCE, input()).is_ok(), "un thread nuovo non richiede l'HUB");
}

#[test]
fn at_most_fifty_routes_per_instance() {
    let f = fixture(source_file());
    for n in 0..MAX_ROUTES_PER_INSTANCE {
        create_route(&f.rt, &world(), INSTANCE, RouteInput { label: format!("r{n}"), ..input() }).unwrap();
    }
    let err = create_route(&f.rt, &world(), INSTANCE, input()).unwrap_err();
    assert!(err.contains("50"), "{err}");
    assert_eq!(f.rt.snapshot().routes.len(), MAX_ROUTES_PER_INSTANCE);
}

#[test]
fn update_changes_the_route_and_keeps_its_identity() {
    let mut file = source_file();
    file.routes.push(route("rte-1-1", RouteThread::NewPerItem, RouteWake::None));
    let f = fixture(file);
    let updated =
        update_route(&f.rt, &world(), "rte-1-1", RouteInput { label: "Altra".to_string(), ..existing("thr-1") })
            .unwrap();
    assert_eq!((updated.id.as_str(), updated.label.as_str()), ("rte-1-1", "Altra"));
    assert_eq!(updated.thread, RouteThread::Existing { thread_id: "thr-1".to_string() });
    assert_eq!((updated.created_at, updated.updated_at), (1, 1_700_000_000_000));
    assert!(update_route(&f.rt, &world(), "rte-9-9", input()).is_err());
    assert!(update_route(&f.rt, &world(), "rte-1-1", existing("thr-2")).is_err());
    assert_eq!(f.rt.snapshot().routes[0].label, "Altra", "invariata dopo il rifiuto");
}

#[test]
fn renaming_with_the_hub_off_does_not_look_the_thread_up_again() {
    let mut file = source_file();
    let mut r = route("rte-1-1", RouteThread::Existing { thread_id: "thr-1".to_string() }, RouteWake::None);
    r.label = "Vecchia".to_string();
    file.routes.push(r);
    let f = fixture(file);
    let off = FakeWorld { hub_off: true, ..world() };
    let renamed = update_route(&f.rt, &off, "rte-1-1", RouteInput { label: "Nuova".to_string(), ..existing("thr-1") });
    assert_eq!(renamed.unwrap().label, "Nuova");
    assert!(update_route(&f.rt, &off, "rte-1-1", existing("thr-2")).is_err(), "cambiare thread la richiede");
}

#[test]
fn re_enabling_a_route_disabled_for_a_gone_thread_checks_the_thread() {
    let mut file = source_file();
    let mut r = route("rte-1-1", RouteThread::Existing { thread_id: "thr-gone".to_string() }, RouteWake::None);
    r.enabled = false;
    r.error = Some("thread non più disponibile".to_string());
    file.routes.push(r);
    let f = fixture(file);
    assert!(update_route(&f.rt, &world(), "rte-1-1", existing("thr-gone")).is_err());
    let fixed = update_route(&f.rt, &world(), "rte-1-1", existing("thr-1")).unwrap();
    assert!(fixed.enabled && fixed.error.is_none());
}

#[test]
fn deleting_a_route_and_a_link() {
    let mut file = source_file();
    file.routes.push(route("rte-1-1", RouteThread::NewPerItem, RouteWake::None));
    file.links.push(link("lnk-1-1", "acc", None));
    let f = fixture(file);
    assert_eq!(list_links(&f.rt, INSTANCE).unwrap().len(), 1);
    assert!(list_links(&f.rt, "non valido").is_err());
    delete_route(&f.rt, "rte-1-1").unwrap();
    assert!(delete_route(&f.rt, "rte-1-1").is_err() && delete_route(&f.rt, "x").is_err());
    delete_link(&f.rt, "lnk-1-1").unwrap();
    assert!(delete_link(&f.rt, "lnk-1-1").is_err());
    let file = f.rt.snapshot();
    assert!(file.routes.is_empty() && file.links.is_empty() && file.instances.len() == 1);
}
