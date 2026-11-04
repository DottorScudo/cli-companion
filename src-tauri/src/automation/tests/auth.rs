//! Autenticazione per sessione: ogni richiesta passa da `rpc::handle` con le
//! credenziali emesse davvero da `AutomationState::issue_credential`.

use super::*;

const DENIED: &str = "invalid session credentials";
/// Id plausibile di un worker mai esistito (stesso formato di quelli veri).
const INVENTED_WORKER: &str = "auto-1700000000000-1";

fn request(session: &str, secret: &str, method: &str, params: Value) -> RpcRequest {
    RpcRequest { rpc: 1, token: secret.to_string(), session: session.to_string(), method: method.to_string(), params }
}

/// RPC con id e segreto della credenziale.
fn send(st: &AutomationState, h: &FakeHost, cred: &SessionCredential, method: &str, params: Value) -> RpcResponse {
    rpc::handle(st, h, &request(cred.session(), cred.secret(), method, params))
}

fn assert_denied(resp: &RpcResponse) {
    assert!(!resp.ok, "{resp:?}");
    assert_eq!(resp.error.as_deref(), Some(DENIED));
}

fn error_of(resp: RpcResponse) -> String {
    assert!(!resp.ok, "attesa una risposta d'errore");
    resp.error.unwrap_or_default()
}

#[test]
fn a_session_cannot_use_the_secret_of_another() {
    let (st, h, w) = running(DeliveryMode::Commit, false);
    let intake = st.issue_credential(INTAKE).unwrap();
    let worker = st.issue_credential(&w).unwrap();
    // Il worker con il segreto dell'Intake e viceversa.
    assert_denied(&rpc::handle(&st, &h, &request(&w, intake.secret(), "get_task", json!({}))));
    assert_denied(&rpc::handle(&st, &h, &request(INTAKE, worker.secret(), "list_tasks", json!({}))));
    // Con il proprio segreto passano entrambi.
    assert!(send(&st, &h, &worker, "get_task", json!({})).ok);
    assert!(send(&st, &h, &intake, "list_tasks", json!({})).ok);
}

#[test]
fn an_invented_session_id_is_rejected_even_with_a_real_secret() {
    let (st, h, _w) = running(DeliveryMode::Commit, false);
    let intake = st.issue_credential(INTAKE).unwrap();
    for secret in [intake.secret(), "", "0123456789abcdef"] {
        assert_denied(&rpc::handle(&st, &h, &request(INVENTED_WORKER, secret, "get_task", json!({}))));
    }
}

#[test]
fn a_worker_with_its_own_credential_cannot_act_as_the_intake() {
    let (st, h, w) = running(DeliveryMode::Commit, false);
    let worker = st.issue_credential(&w).unwrap();
    let session_only = Method::ALL.into_iter().filter(|m| m.allows(Role::Session) && !m.allows(Role::Worker));
    for method in session_only {
        let e = error_of(send(&st, &h, &worker, method.name(), json!({})));
        assert!(e.contains("not available to automated sessions"), "{}: {e}", method.name());
    }
    assert!(send(&st, &h, &worker, "get_task", json!({})).ok);
    assert_eq!(send(&st, &h, &worker, "whoami", json!({})).result["role"], "worker");
}

#[test]
fn the_intake_with_its_own_credential_manages_tasks_but_not_worker_tools() {
    let (st, h, _w) = running(DeliveryMode::Commit, false);
    let intake = st.issue_credential(INTAKE).unwrap();
    let proposal = send(&st, &h, &intake, "propose_tasks", json!({"tasks": [spec("x", &[])]}));
    assert!(proposal.ok, "{proposal:?}");
    assert!(send(&st, &h, &intake, "list_tasks", json!({})).ok);
    for method in ["get_task", "complete_task"] {
        let e = error_of(send(&st, &h, &intake, method, json!({})));
        assert!(e.contains("reserved for automated sessions"), "{method}: {e}");
    }
}

#[test]
fn a_revoked_credential_is_rejected() {
    let (st, h, w) = running(DeliveryMode::Commit, false);
    let worker = st.issue_credential(&w).unwrap();
    let (session, secret) = (worker.session().to_string(), worker.secret().to_string());
    assert!(rpc::handle(&st, &h, &request(&session, &secret, "get_task", json!({}))).ok);
    drop(worker);
    assert_denied(&rpc::handle(&st, &h, &request(&session, &secret, "get_task", json!({}))));
}

#[test]
fn a_reopened_tab_invalidates_the_secret_of_the_previous_terminal() {
    let (st, h, w) = running(DeliveryMode::Commit, false);
    let old = st.issue_credential(&w).unwrap();
    let new = st.issue_credential(&w).unwrap();
    assert_denied(&send(&st, &h, &old, "get_task", json!({})));
    assert!(send(&st, &h, &new, "get_task", json!({})).ok);
    // La chiusura in ritardo del vecchio terminale non toglie il segreto del nuovo.
    drop(old);
    assert!(send(&st, &h, &new, "get_task", json!({})).ok);
}

#[test]
fn malformed_requests_are_rejected_before_dispatch() {
    let (st, h, _w) = running(DeliveryMode::Commit, false);
    let intake = st.issue_credential(INTAKE).unwrap();
    let mut wrong_version = request(INTAKE, intake.secret(), "list_tasks", json!({}));
    wrong_version.rpc = 2;
    assert_eq!(rpc::handle(&st, &h, &wrong_version).error.as_deref(), Some("unsupported RPC version"));
    for (session, secret) in [(INTAKE, ""), ("a b", intake.secret()), ("", intake.secret())] {
        assert_denied(&rpc::handle(&st, &h, &request(session, secret, "list_tasks", json!({}))));
    }
}
