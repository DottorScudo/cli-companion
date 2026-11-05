use super::*;

fn list(route: &str, ids: &[&str]) -> (String, Vec<String>) {
    (route.to_string(), ids.iter().map(|s| s.to_string()).collect())
}

fn pairs(plan: &[(String, String)]) -> Vec<(&str, &str)> {
    plan.iter().map(|(r, i)| (r.as_str(), i.as_str())).collect()
}

#[test]
fn routes_take_turns_oldest_first() {
    let lists = [list("a", &["a1", "a2", "a3"]), list("b", &["b1"])];
    let (plan, more) = plan_fetch(&lists, 10);
    assert_eq!(pairs(&plan), vec![("a", "a1"), ("b", "b1"), ("a", "a2"), ("a", "a3")]);
    assert!(!more);
}

#[test]
fn a_busy_route_cannot_starve_the_others() {
    let busy: Vec<String> = (0..100).map(|n| format!("a{n}")).collect();
    let lists = [("a".to_string(), busy), list("b", &["b1"]), list("c", &["c1"])];
    let (plan, more) = plan_fetch(&lists, 3);
    assert_eq!(pairs(&plan), vec![("a", "a0"), ("b", "b1"), ("c", "c1")]);
    assert!(more);
}

#[test]
fn the_budget_cuts_mid_round_and_reports_more() {
    let lists = [list("a", &["a1", "a2"]), list("b", &["b1", "b2"])];
    let (plan, more) = plan_fetch(&lists, 3);
    assert_eq!(pairs(&plan), vec![("a", "a1"), ("b", "b1"), ("a", "a2")]);
    assert!(more);
}

#[test]
fn an_exact_budget_leaves_nothing_out() {
    let lists = [list("a", &["a1"]), list("b", &["b1"])];
    let (plan, more) = plan_fetch(&lists, 2);
    assert_eq!(plan.len(), 2);
    assert!(!more);
}

#[test]
fn empty_inputs_and_zero_budget() {
    assert_eq!(plan_fetch(&[], 5), (vec![], false));
    assert_eq!(plan_fetch(&[list("a", &[])], 5), (vec![], false));
    let (plan, more) = plan_fetch(&[list("a", &["a1"])], 0);
    assert!(plan.is_empty());
    assert!(more);
}

#[test]
fn the_same_message_for_two_routes_is_planned_for_both() {
    let lists = [list("a", &["m1"]), list("b", &["m1"])];
    let (plan, _) = plan_fetch(&lists, 10);
    assert_eq!(pairs(&plan), vec![("a", "m1"), ("b", "m1")]);
}

#[test]
fn a_huge_budget_is_fine() {
    let (plan, more) = plan_fetch(&[list("a", &["a1"])], usize::MAX);
    assert_eq!(plan.len(), 1);
    assert!(!more);
}
