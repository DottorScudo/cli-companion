use super::*;
use crate::board::Task;
use crate::hub::model::TaskField::{DependsOn, Model, Task as Spec, Title};

fn task() -> Task {
    let spec = TaskSpec {
        id: "t-1".into(),
        title: "Login".into(),
        task: "Add a login page.".into(),
        depends_on: vec!["a".into(), "b".into()],
        model: Some("sonnet".into()),
        ..TaskSpec::default()
    };
    Task::from_spec(spec, 0)
}

fn fields(update: &TaskUpdate) -> Vec<TaskField> {
    update.changes.iter().map(|c| c.field).collect()
}

#[test]
fn only_the_fields_that_differ_become_changes_in_the_card_order() {
    let patch =
        json!({ "model": "opus", "title": "Login page", "task": "Add a login page.", "depends_on": ["a", "b"] });
    let update = build(&task(), &patch).unwrap();
    assert_eq!(fields(&update), [Title, Model]);
    assert_eq!(update.changes[0].before.as_deref(), Some("Login"));
    assert_eq!(update.changes[0].after.as_deref(), Some("Login page"));
    assert_eq!(update.patch, json!({ "title": "Login page", "model": "opus" }));
    assert_eq!(update.base, json!({ "title": "Login", "model": "sonnet" }));
}

#[test]
fn dependencies_are_listed_and_an_empty_value_is_none() {
    let update = build(&task(), &json!({ "depends_on": ["c"], "model": null })).unwrap();
    assert_eq!(fields(&update), [DependsOn, Model]);
    assert_eq!(update.changes[0].before.as_deref(), Some("a, b"));
    assert_eq!(update.changes[0].after.as_deref(), Some("c"));
    assert_eq!((update.changes[1].before.as_deref(), update.changes[1].after.clone()), (Some("sonnet"), None));
    let cleared = build(&task(), &json!({ "depends_on": [] })).unwrap();
    assert_eq!(cleared.changes[0].after, None);
    assert_eq!(cleared.patch, json!({ "depends_on": [] }));
}

#[test]
fn the_card_cuts_a_long_specification_but_the_patch_keeps_all_of_it() {
    let long = format!("{}\nend", "x".repeat(PREVIEW_KEPT * 3));
    let update = build(&task(), &json!({ "task": long })).unwrap();
    assert_eq!(fields(&update), [Spec]);
    let shown = update.changes[0].after.as_deref().unwrap();
    assert!(shown.ends_with('…') && shown.chars().count() <= PREVIEW_KEPT + 1, "{}", shown.chars().count());
    assert_eq!(update.patch["task"], json!(long));
    assert_eq!(update.base["task"], "Add a login page.");
}

#[test]
fn a_patch_without_differences_or_with_other_fields_is_refused() {
    let same = build(&task(), &json!({ "title": "  Login  " })).unwrap_err();
    assert!(same.contains("nothing to change"), "{same}");
    assert!(build(&task(), &json!({})).unwrap_err().contains("nothing to change"));
    for other in [json!({ "repo": "x" }), json!({ "kind": "review" }), json!({ "e2e": true }), json!({ "id": "t-1" })] {
        let e = build(&task(), &other).unwrap_err();
        assert!(e.starts_with("Only `title`"), "{other}: {e}");
    }
}

#[test]
fn malformed_values_are_refused_with_the_board_error() {
    for (bad, expected) in [
        (json!({ "title": "x".repeat(200) }), "title is too long"),
        (json!({ "depends_on": ["Not Valid"] }), "invalid task id"),
        (json!({ "model": "bad model!" }), "invalid model"),
        (json!({ "task": 5 }), "task must be a string"),
    ] {
        let e = build(&task(), &bad).unwrap_err();
        assert!(e.contains(expected), "{bad}: {e}");
    }
    assert!(build(&task(), &json!("title")).is_err());
}

#[test]
fn the_base_matches_until_a_changed_field_is_edited_elsewhere() {
    let update = build(&task(), &json!({ "title": "Login page" })).unwrap();
    assert!(base_matches(&update, &task()));
    let mut other = task();
    other.title = "Sign in".into();
    assert!(!base_matches(&update, &other));
    let mut unrelated = task();
    unrelated.task = "Something else".into();
    assert!(base_matches(&update, &unrelated), "a field the request does not touch does not matter");
}
