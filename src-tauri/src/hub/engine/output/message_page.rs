//! La pagina di messaggi di `thread_read` (pura), che sta sempre in
//! `TOOL_RESULT_MAX` misurata sul JSON che il modello riceve. Parte dal
//! messaggio piu' recente della finestra e va all'indietro: tiene quelli che
//! stanno nel limite insieme a intestazione e campi di paginazione, cosi'
//! `older_messages`/`next_before` puntano al primo messaggio escluso e `before`
//! legge il resto senza buchi ne' ripetizioni. Un messaggio che da solo non sta
//! si accorcia (mai una pagina vuota): il testo resta `body_truncated`, leggibile
//! intero con `message_id`.

use super::super::turn::TOOL_RESULT_MAX;
use super::task_details::pieces::{size, Content};
use super::text_cursor::MARKER_KEYS;
use super::{header, message, BODY_KEPT};
use crate::hub::engine::workspace_names::WorkspaceNames;
use crate::hub::model::{Message, Thread};
use serde_json::{json, Map, Value};
use std::ops::Range;

/// Il segno di un testo tagliato, come in `clip_with_ellipsis`.
const ELLIPSIS: &str = "…";
/// I campi di un evento che restano quando i suoi dati non stanno nella pagina:
/// gli identificativi (i dati completi del task sono in `task_details`); vi
/// restano anche i cursori dei testi accorciati (`text_cursor::MARKER_KEYS`).
const EVENT_ID_FIELDS: [&str; 6] = ["kind", "workspace", "workspace_name", "task_id", "proposal_id", "status"];
/// Gli oggetti di un evento di cui resta solo l'`id` (per `relay_answer` e le richieste).
const EVENT_ID_OBJECTS: [&str; 2] = ["question", "request"];

/// Intestazione del thread e i messaggi piu' recenti di `window` che stanno nel
/// limite, con `older_messages`/`next_before` se restano messaggi prima e
/// `newer_messages` se ne restano dopo la finestra.
pub fn message_page(thread: &Thread, names: &WorkspaceNames, window: Range<usize>) -> Value {
    let all = &thread.messages;
    let mut out = header(thread, names);
    out["messages"] = Value::Array(Vec::new());
    // I campi di paginazione occupano al massimo questo: i valori veri non sono piu' lunghi.
    reserve_paging(&mut out, all, &window);
    let room = TOOL_RESULT_MAX.saturating_sub(size(&out));
    let kept = newest_that_fit(&all[window.clone()], names, room);
    let start = window.end - kept.len();
    clear_paging(&mut out);
    out["messages"] = Value::Array(kept);
    if start > 0 {
        out["older_messages"] = Value::from(start);
        out["next_before"] = Value::from(all[start].id.as_str());
    }
    if window.end < all.len() {
        out["newer_messages"] = Value::from(all.len() - window.end);
    }
    out
}

/// I campi di paginazione con i valori piu' lunghi che possono avere: `start`
/// non supera la fine della finestra e `next_before` e' l'id di un suo messaggio.
fn reserve_paging(out: &mut Value, all: &[Message], window: &Range<usize>) {
    let longest_id = all[window.clone()].iter().map(|m| m.id.as_str()).max_by_key(|id| size(&Value::from(*id)));
    if let Some(id) = longest_id {
        out["older_messages"] = Value::from(window.end);
        out["next_before"] = Value::from(id);
    }
    if window.end < all.len() {
        out["newer_messages"] = Value::from(all.len() - window.end);
    }
}

fn clear_paging(out: &mut Value) {
    if let Some(fields) = out.as_object_mut() {
        for key in ["older_messages", "next_before", "newer_messages"] {
            fields.remove(key);
        }
    }
}

/// Dal piu' recente, i messaggi di `list` che stanno in `room` caratteri (con le
/// virgole dell'elenco), nell'ordine del thread. Se nemmeno l'ultimo sta, c'e'
/// lui accorciato.
fn newest_that_fit(list: &[Message], names: &WorkspaceNames, room: usize) -> Vec<Value> {
    let mut kept = Vec::new();
    let mut used = 0;
    for m in list.iter().rev() {
        let value = message(m, names);
        let cost = size(&value) + usize::from(!kept.is_empty());
        if used + cost > room {
            if kept.is_empty() {
                kept.push(shortened(m, names, room));
            }
            break;
        }
        used += cost;
        kept.push(value);
    }
    kept.reverse();
    kept
}

/// Il messaggio `m` in `room` caratteri: il testo si accorcia quanto serve; se
/// non basta, dell'evento restano solo gli identificativi (`event_truncated`).
fn shortened(m: &Message, names: &WorkspaceNames, room: usize) -> Value {
    let total = m.body.chars().count();
    let mut out = message(m, names);
    out["body"] = Value::from("");
    out["body_truncated"] = json!({ "total_chars": total });
    let ellipsis = ELLIPSIS.chars().count();
    if size(&out) + ellipsis > room {
        if let Some(event) = out.get("event").map(event_ids) {
            out["event"] = event;
            out["event_truncated"] = Value::Bool(true);
        }
    }
    let head: String = m.body.chars().take(BODY_KEPT).collect();
    let piece = Content::Text(&head).piece(0, room.saturating_sub(size(&out) + ellipsis));
    let kept = piece.value.as_str().unwrap_or_default();
    if piece.end == total {
        out["body"] = Value::from(kept);
        if let Some(fields) = out.as_object_mut() {
            fields.remove("body_truncated");
        }
    } else {
        out["body"] = Value::from(format!("{kept}{ELLIPSIS}"));
    }
    out
}

/// Gli identificativi dell'evento `event` (vedi `EVENT_ID_FIELDS` e `EVENT_ID_OBJECTS`).
fn event_ids(event: &Value) -> Value {
    let mut ids: Map<String, Value> = EVENT_ID_FIELDS
        .iter()
        .chain(&MARKER_KEYS)
        .filter_map(|key| Some((key.to_string(), event.get(key)?.clone())))
        .collect();
    for key in EVENT_ID_OBJECTS {
        if let Some(id) = event.get(key).and_then(|object| object.get("id")) {
            ids.insert(key.to_string(), json!({ "id": id }));
        }
    }
    Value::Object(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::engine::fixtures::{self, msg, names};
    use crate::hub::engine::output::thread;
    use crate::hub::engine::thread_page::{ThreadPage, THREAD_PAGE_SIZE};
    use crate::hub::model::{Author, HubEvent, HubEventKind, HubQuestion, MessageKind};

    /// Un testo di `n` caratteri con virgolette, barre, a capo e caratteri multibyte.
    fn awkward(n: usize, seed: usize) -> String {
        let unit: Vec<char> = "àè\"\\ ok\n\tリ🙂 ".chars().collect();
        (0..n).map(|i| unit[(i + seed) % unit.len()]).collect()
    }

    fn event_message(id: &str, body: &str, detail: &str) -> Message {
        let mut m = msg(id, Author::System, body, 1_700_000_000_000);
        m.kind = MessageKind::Event;
        m.event = Some(HubEvent {
            workspace: Some("ws-2".into()),
            status: Some("failed".into()),
            summary: Some(detail.into()),
            note: Some(detail.into()),
            question: Some(HubQuestion { id: "q-1".into(), text: detail.into(), options: vec![detail.into()] }),
            ..HubEvent::of_task(HubEventKind::TaskFailed, "t-7")
        });
        m
    }

    /// 50 messaggi lunghi, uno su cinque un evento con i suoi dati.
    fn long_thread() -> Thread {
        let messages = (0..50)
            .map(|i| {
                let body = awkward(1000 + i * 7, i);
                match i % 5 {
                    0 => event_message(&format!("m{i}"), &body, &awkward(300, i)),
                    1 => fixtures::external(&format!("m{i}"), "gmail", "Anna \"A\" <a@example.com>", &body),
                    _ => msg(&format!("m{i}"), Author::User, &body, i as i64),
                }
            })
            .collect();
        Thread { summary: Some(awkward(1500, 3)), ..fixtures::thread(messages) }
    }

    fn ids(page: &Value) -> Vec<String> {
        page["messages"].as_array().unwrap().iter().map(|m| m["id"].as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn following_next_before_reads_every_long_message_once_in_pages_within_the_limit() {
        let t = long_thread();
        let mut page = thread(&t, &names(), &ThreadPage::Latest).unwrap();
        let mut pages = Vec::new();
        loop {
            assert!(size(&page) <= TOOL_RESULT_MAX, "page {} is {} long", pages.len(), size(&page));
            assert!(!ids(&page).is_empty() && ids(&page).len() < THREAD_PAGE_SIZE);
            let next = page["next_before"].as_str().map(str::to_string);
            assert_eq!(
                page["older_messages"].as_u64(),
                next.as_ref().map(|_| t.messages.len() as u64 - read(&pages, &page))
            );
            pages.push(ids(&page));
            match next {
                Some(id) => page = thread(&t, &names(), &ThreadPage::Before(id)).unwrap(),
                None => break,
            }
        }
        let read: Vec<String> = pages.into_iter().rev().flatten().collect();
        let all: Vec<String> = t.messages.iter().map(|m| m.id.clone()).collect();
        assert_eq!(read, all);
    }

    /// Messaggi letti finora, contando la pagina `page`.
    fn read(pages: &[Vec<String>], page: &Value) -> u64 {
        (pages.iter().map(Vec::len).sum::<usize>() + ids(page).len()) as u64
    }

    #[test]
    fn a_long_page_keeps_the_newest_messages_and_their_markers_and_counts() {
        let t = long_thread();
        let v = thread(&t, &names(), &ThreadPage::Offset(10)).unwrap();
        let list = ids(&v);
        assert_eq!(list.last().map(String::as_str), Some("m39"));
        assert_eq!(v["newer_messages"], 10);
        assert_eq!(v["next_before"].as_str(), Some(list[0].as_str()));
        assert_eq!(v["summary"].as_str(), t.summary.as_deref());
        let last = v["messages"].as_array().unwrap().last().unwrap();
        assert_eq!(last["body_truncated"]["total_chars"], 1000 + 39 * 7);
        assert!(v["messages"].as_array().unwrap().iter().all(|m| m.get("event_truncated").is_none()));
    }

    #[test]
    fn a_huge_event_alone_is_shortened_within_the_limit_and_keeps_its_ids() {
        let huge = event_message("e1", &awkward(20_000, 0), &awkward(20_000, 1));
        let t = Thread { summary: Some(awkward(1500, 2)), ..fixtures::thread(vec![huge]) };
        let v = thread(&t, &names(), &ThreadPage::Latest).unwrap();
        assert!(size(&v) <= TOOL_RESULT_MAX, "{}", size(&v));
        let m = &v["messages"][0];
        assert_eq!(m["body_truncated"]["total_chars"], 20_000);
        assert!(m["body"].as_str().unwrap().ends_with(ELLIPSIS));
        assert_eq!(m["event_truncated"], true);
        assert_eq!(
            m["event"],
            json!({
                "kind": "task_failed", "workspace": "ws-2", "workspace_name": "Sito Web",
                "task_id": "t-7", "status": "failed", "question": { "id": "q-1" }
            })
        );
        assert!(v.get("older_messages").is_none() && v.get("newer_messages").is_none());
    }

    #[test]
    fn an_event_that_fits_once_its_text_is_shortened_keeps_all_its_data() {
        let m = event_message("e1", &awkward(5000, 0), &awkward(900, 1));
        assert!(size(&message(&m, &names())) > TOOL_RESULT_MAX, "the whole message must not fit");
        let t = fixtures::thread(vec![msg("m0", Author::User, "prima", 0), m.clone()]);
        let v = thread(&t, &names(), &ThreadPage::Latest).unwrap();
        assert!(size(&v) <= TOOL_RESULT_MAX, "{}", size(&v));
        assert_eq!(ids(&v), ["e1"]);
        let kept = &v["messages"][0];
        assert_eq!(kept["body_truncated"]["total_chars"], 5000);
        assert!(kept["body"].as_str().unwrap().chars().count() < BODY_KEPT);
        assert!(kept.get("event_truncated").is_none());
        assert_eq!(kept["event"]["question"]["text"], json!(m.event.unwrap().question.unwrap().text));
        assert_eq!((v["older_messages"].as_u64(), v["next_before"].as_str()), (Some(1), Some("e1")));
    }

    #[test]
    fn an_empty_thread_has_no_messages_and_no_paging() {
        let v = thread(&fixtures::thread(Vec::new()), &names(), &ThreadPage::Latest).unwrap();
        assert_eq!(v["messages"], json!([]));
        assert!(v.get("older_messages").is_none() && v.get("next_before").is_none());
    }
}
