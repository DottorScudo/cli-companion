use super::*;

fn get(target: &str) -> Vec<u8> {
    format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nUser-Agent: x\r\n\r\n").into_bytes()
}

#[test]
fn the_head_ends_after_the_blank_line() {
    assert_eq!(head_end(b"GET / HTTP/1.1\r\n\r\n"), Some(18));
    assert_eq!(head_end(b"GET / HTTP/1.1\r\n\r\nbody"), Some(18));
    assert_eq!(head_end(b"GET / HTTP/1.1\r\nHost: x\r\n"), None);
    assert_eq!(head_end(b""), None);
    assert_eq!(head_end(b"\r\n\r\n"), Some(4));
    assert_eq!(head_end(b"\r\n\r"), None);
}

#[test]
fn a_successful_redirect_yields_code_and_state() {
    let parsed = parse_request(&get("/?state=st-1_A&code=4%2F0AbC&scope=x&authuser=0")).unwrap();
    assert_eq!(parsed, Callback::Code { code: "4/0AbC".to_string(), state: "st-1_A".to_string() });
}

#[test]
fn parameter_order_and_plus_signs_do_not_matter() {
    let parsed = parse_request(&get("/?code=a+b&state=s")).unwrap_err();
    assert_eq!(parsed, CallbackError::Malformed, "uno spazio non e' un codice valido");
    let ok = parse_request(&get("/?scope=a&code=c1&state=s1")).unwrap();
    assert_eq!(ok, Callback::Code { code: "c1".to_string(), state: "s1".to_string() });
}

#[test]
fn a_denied_consent_is_reported_with_its_state() {
    assert_eq!(
        parse_request(&get("/?error=access_denied&state=abc")).unwrap(),
        Callback::Denied { state: Some("abc".to_string()) }
    );
    assert_eq!(parse_request(&get("/?error=access_denied")).unwrap(), Callback::Denied { state: None });
    assert_eq!(parse_request(&get("/?error=access_denied&state=a%20b")).unwrap(), Callback::Denied { state: None });
}

#[test]
fn other_paths_and_bare_root_are_other() {
    for target in ["/favicon.ico", "/", "/?", "/x?code=a&state=b", "/index.html", "//", "*"] {
        assert_eq!(parse_request(&get(target)).unwrap(), Callback::Other, "{target}");
    }
}

#[test]
fn missing_or_invalid_code_or_state_is_malformed() {
    for target in [
        "/?code=abc",
        "/?state=abc",
        "/?code=&state=s",
        "/?code=abc&state=",
        "/?code=%FF&state=s",
        "/?code=a%0Ab&state=s",
        "/?foo=bar",
    ] {
        assert_eq!(parse_request(&get(target)), Err(CallbackError::Malformed), "{target}");
    }
}

#[test]
fn length_limits_apply_to_code_and_state() {
    let long_code = "a".repeat(2049);
    assert!(parse_request(&get(&format!("/?code={long_code}&state=s"))).is_err());
    let max_code = "a".repeat(2048);
    assert!(parse_request(&get(&format!("/?code={max_code}&state=s"))).is_ok());
    let long_state = "s".repeat(257);
    assert!(parse_request(&get(&format!("/?code=c&state={long_state}"))).is_err());
}

#[test]
fn only_a_well_formed_get_is_accepted() {
    for head in [
        &b"POST /?code=a&state=b HTTP/1.1\r\n\r\n"[..],
        b"get /?code=a&state=b HTTP/1.1\r\n\r\n",
        b"GET /?code=a&state=b\r\n\r\n",
        b"GET /?code=a&state=b FTP/1.1\r\n\r\n",
        b"GET /a b HTTP/1.1\r\n\r\n",
        b"",
        b"\r\n\r\n",
        b"\xff\xfe GET / HTTP/1.1\r\n\r\n",
        b"GET",
    ] {
        assert_eq!(parse_request(head), Err(CallbackError::Malformed), "{:?}", String::from_utf8_lossy(head));
    }
}

#[test]
fn a_head_without_terminator_is_still_parsed_by_its_first_line() {
    assert_eq!(parse_request(b"GET /favicon.ico HTTP/1.1").unwrap(), Callback::Other);
}

#[test]
fn unicode_and_huge_inputs_do_not_panic() {
    let huge = "é".repeat(100_000);
    let _ = parse_request(&get(&format!("/?code={huge}&state=s")));
    let _ = parse_request(&get("/?code=😀&state=😀"));
    let _ = parse_request(&vec![0xFF; 100_000]);
}

#[test]
fn responses_are_complete_http_with_a_correct_length() {
    for (page, status, needle) in [
        (Page::Success, "200 OK", "Autorizzazione completata"),
        (Page::Failure, "400 Bad Request", "non riuscita"),
        (Page::NotFound, "404 Not Found", "non trovata"),
    ] {
        let raw = response(page);
        let text = String::from_utf8(raw).unwrap();
        assert!(text.starts_with(&format!("HTTP/1.1 {status}\r\n")), "{text}");
        assert!(text.contains("Content-Type: text/html; charset=utf-8\r\n"));
        assert!(text.contains("Cache-Control: no-store\r\n"));
        assert!(text.contains("Connection: close\r\n"));
        assert!(text.contains(needle));
        let (head, body) = text.split_once("\r\n\r\n").unwrap();
        assert!(head.contains(&format!("Content-Length: {}", body.len())), "{head}");
    }
}

#[test]
fn the_success_page_says_you_can_close_the_window() {
    let text = String::from_utf8(response(Page::Success)).unwrap();
    assert!(text.contains("puoi chiudere questa finestra e tornare all'app."));
}

#[test]
fn the_head_limit_is_eight_kib() {
    assert_eq!(HEAD_MAX, 8192);
}
