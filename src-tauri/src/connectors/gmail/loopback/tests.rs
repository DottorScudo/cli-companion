use super::*;
use tokio::io::duplex;

const STATE: &str = "stato-giusto";

fn run(request: &[u8]) -> (Verdict, String) {
    tauri::async_runtime::block_on(async {
        let (mut client, mut server) = duplex(64 * 1024);
        client.write_all(request).await.unwrap();
        let verdict = handle(&mut server, STATE).await;
        let mut out = Vec::new();
        client.read_to_end(&mut out).await.unwrap();
        (verdict, String::from_utf8_lossy(&out).into_owned())
    })
}

fn get(target: &str) -> Vec<u8> {
    format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").into_bytes()
}

#[test]
fn the_redirect_with_the_right_state_gives_the_code() {
    let (verdict, reply) = run(&get("/?state=stato-giusto&code=4%2Fabc&scope=x"));
    assert_eq!(verdict, Verdict::Code("4/abc".to_string()));
    assert!(reply.starts_with("HTTP/1.1 200 OK"), "{reply}");
    assert!(!reply.contains("4/abc") && !reply.contains(STATE));
}

#[test]
fn a_wrong_or_missing_state_fails_the_authorization() {
    for target in ["/?state=altro&code=abc", "/?state=altro&error=access_denied", "/?error=access_denied"] {
        let (verdict, reply) = run(&get(target));
        assert_eq!(verdict, Verdict::Invalid, "{target}");
        assert!(reply.starts_with("HTTP/1.1 400"), "{reply}");
    }
}

#[test]
fn a_denial_with_the_right_state_is_a_denial() {
    let (verdict, reply) = run(&get("/?state=stato-giusto&error=access_denied"));
    assert_eq!(verdict, Verdict::Denied);
    assert!(reply.starts_with("HTTP/1.1 400"), "{reply}");
}

#[test]
fn other_requests_get_a_404_and_the_wait_continues() {
    for request in [get("/favicon.ico"), get("/"), b"POST / HTTP/1.1\r\n\r\n".to_vec(), b"garbage\r\n\r\n".to_vec()] {
        let (verdict, reply) = run(&request);
        assert_eq!(verdict, Verdict::KeepWaiting);
        assert!(reply.starts_with("HTTP/1.1 404"), "{reply}");
    }
}

#[test]
fn a_huge_head_without_terminator_is_dropped() {
    let (verdict, reply) = run(&vec![b'a'; HEAD_MAX + 4096]);
    assert_eq!(verdict, Verdict::KeepWaiting);
    assert!(reply.starts_with("HTTP/1.1 404"), "{reply}");
}

#[test]
fn a_connection_closed_early_keeps_waiting() {
    let verdict = tauri::async_runtime::block_on(async {
        let (client, mut server) = duplex(1024);
        drop(client);
        handle(&mut server, STATE).await
    });
    assert_eq!(verdict, Verdict::KeepWaiting);
}

#[test]
fn a_split_head_is_reassembled() {
    let verdict = tauri::async_runtime::block_on(async {
        let (mut client, mut server) = duplex(1024);
        client.write_all(b"GET /?state=stato-giusto&co").await.unwrap();
        let writer = async {
            client.write_all(b"de=xyz HTTP/1.1\r\n\r\n").await.unwrap();
        };
        let (verdict, ()) = tokio::join!(handle(&mut server, STATE), writer);
        verdict
    });
    assert_eq!(verdict, Verdict::Code("xyz".to_string()));
}

#[test]
fn the_wait_leaves_room_for_the_exchange() {
    assert!(AUTH_WAIT < ACTION_TIMEOUT);
}
