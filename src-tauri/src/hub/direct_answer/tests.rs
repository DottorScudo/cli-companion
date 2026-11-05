use super::*;
use crate::hub::model::{Attachment, HubEventKind};
use crate::hub::questions::fixtures::{question_event, question_ref, task_event, thread, user, FakeAnswers};
use std::path::Path;

#[test]
fn a_reply_to_a_pending_question_reaches_the_worker_as_written() {
    let body = "  Usa Postgres, non SQLite.\nGrazie  ";
    let reply = user("m1", body, Some("e1"));
    let t = thread(vec![question_event("e1", "a", "q-1-1"), reply.clone()]);
    let board = FakeAnswers::default();
    assert_eq!(answer_reply(&t, &reply, &board, Path::new("root")).as_deref(), Some(ANSWER_SENT));
    assert_eq!(board.delivered(), [(question_ref("a", "q-1-1"), body.to_string())]);
}

#[test]
fn a_delivery_error_is_reported_with_the_terminal_hint() {
    let reply = user("m1", "sì", Some("e1"));
    let t = thread(vec![question_event("e1", "a", "q-1-1"), reply.clone()]);
    let board = FakeAnswers::refusing("Il worker sta ancora lavorando.");
    let outcome = answer_reply(&t, &reply, &board, Path::new("root")).unwrap();
    assert_eq!(outcome, format!("Risposta non inviata: Il worker sta ancora lavorando. {TERMINAL_HINT}"));
}

#[test]
fn a_question_that_no_longer_waits_is_a_clear_error_without_delivery() {
    let board = FakeAnswers::default();
    for next in [HubEventKind::TaskQuestionAnswered, HubEventKind::TaskDone, HubEventKind::TaskFailed] {
        let reply = user("m1", "sì", Some("e1"));
        let t = thread(vec![question_event("e1", "a", "q-1-1"), task_event("e2", next, "a"), reply.clone()]);
        assert_eq!(answer_reply(&t, &reply, &board, Path::new("root")).as_deref(), Some(NOT_PENDING), "{next:?}");
    }
    assert!(board.delivered().is_empty());
}

#[test]
fn other_messages_follow_the_normal_path() {
    let board = FakeAnswers::default();
    let t = thread(vec![question_event("e1", "a", "q-1-1"), user("m0", "ciao", None)]);
    assert_eq!(answer_reply(&t, &user("m1", "@mario stato?", None), &board, Path::new("root")), None);
    assert_eq!(answer_reply(&t, &user("m2", "ok", Some("m0")), &board, Path::new("root")), None);
    let mut by_bot = user("m3", "sì", Some("e1"));
    by_bot.author = Author::Bot { name: "mario".into() };
    assert_eq!(answer_reply(&t, &by_bot, &board, Path::new("root")), None, "solo l'utente risponde direttamente");
    assert!(board.delivered().is_empty());
}

fn attachment(id: &str, name: &str) -> Attachment {
    Attachment { id: id.to_string(), name: name.to_string(), mime: "image/png".to_string(), size: 2048 }
}

#[test]
fn the_attachments_of_a_reply_reach_the_worker_as_absolute_paths_after_the_text() {
    let mut reply = user("m1", "Ecco lo screenshot.", Some("e1"));
    reply.attachments = vec![attachment("att-1-1", "errore.png"), attachment("att-1-2", "log.png")];
    let t = thread(vec![question_event("e1", "a", "q-1-1"), reply.clone()]);
    let board = FakeAnswers::default();
    let root = Path::new("data").join("hub_attachments");
    assert_eq!(answer_reply(&t, &reply, &board, &root).as_deref(), Some(ANSWER_SENT));
    let stored = |id: &str, name: &str| root.join("channels").join("ws-1").join("t1").join(id).join(name);
    let expected = format!(
        "Ecco lo screenshot.\n\n{ATTACHMENTS_HEADER}\n- {} — errore.png (image/png, 2.0 KB)\n- {} — log.png (image/png, 2.0 KB)",
        stored("att-1-1", "errore.png").display(),
        stored("att-1-2", "log.png").display()
    );
    assert_eq!(board.delivered(), [(question_ref("a", "q-1-1"), expected)]);
}

#[test]
fn a_reply_with_only_attachments_delivers_just_the_list() {
    let mut reply = user("m1", "", Some("e1"));
    reply.attachments = vec![attachment("att-1-1", "errore.png")];
    let t = thread(vec![question_event("e1", "a", "q-1-1"), reply.clone()]);
    let board = FakeAnswers::default();
    answer_reply(&t, &reply, &board, Path::new("root")).unwrap();
    let delivered = board.delivered();
    assert!(delivered[0].1.starts_with(ATTACHMENTS_HEADER) && delivered[0].1.contains("errore.png"), "{delivered:?}");
}
