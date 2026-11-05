//! Il testo di un solo messaggio per `thread_read` con `message_id` (puro): un
//! pezzo per volta, grande quanto `TOOL_RESULT_MAX` permette, con `next_offset`
//! finche' il testo continua. Le pagine di messaggi ne portano solo l'inizio
//! (`body_truncated`); qui il testo si ricostruisce per intero.

use super::super::turn::TOOL_RESULT_MAX;
use super::task_details::pieces::{size, Content};
use crate::hub::model::{Author, Message};
use serde_json::{json, Value};

/// Il pezzo del testo del messaggio `message_id` che parte dal carattere
/// `offset`; l'errore e' in inglese, per il modello.
pub fn message_body(messages: &[Message], message_id: &str, offset: usize) -> Result<Value, String> {
    let message = messages
        .iter()
        .find(|m| m.id == message_id)
        .ok_or_else(|| "`message_id` is not the id of a message of this thread".to_string())?;
    let content = Content::Text(&message.body);
    let total = content.total();
    if offset > 0 && offset >= total {
        return Err(format!("`offset` {offset} is beyond the end of the message, which has {total} characters."));
    }
    let mut out = json!({ "message_id": message.id, "offset": offset, "total_chars": total, "content": "" });
    // Il testo di un autore esterno e' un dato non fidato, anche a pezzi.
    if matches!(message.author, Author::External { .. }) {
        out["untrusted"] = Value::Bool(true);
    }
    out["next_offset"] = Value::from(total);
    let piece = content.piece(offset, TOOL_RESULT_MAX.saturating_sub(size(&out)));
    out["content"] = piece.value;
    if piece.end < total {
        out["next_offset"] = Value::from(piece.end);
    } else if let Some(fields) = out.as_object_mut() {
        fields.remove("next_offset");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::engine::fixtures::{self, msg};

    fn read_all(messages: &[Message], id: &str) -> (String, usize) {
        let (mut text, mut offset, mut parts) = (String::new(), 0, 0);
        loop {
            let v = message_body(messages, id, offset).unwrap();
            assert!(size(&v) <= TOOL_RESULT_MAX, "part {parts} is {} long", size(&v));
            text.push_str(v["content"].as_str().unwrap());
            parts += 1;
            match v["next_offset"].as_u64() {
                Some(next) => offset = next as usize,
                None => return (text, parts),
            }
        }
    }

    #[test]
    fn the_parts_of_a_long_message_rebuild_it_exactly() {
        let unit = "àè\"\\ ok\n\tリ🙂 ";
        let body = unit.repeat(800);
        assert_eq!(body.chars().count(), 800 * unit.chars().count());
        let messages = [msg("m1", Author::User, &body, 0)];
        let (text, parts) = read_all(&messages, "m1");
        assert_eq!(text, body);
        assert!(parts > 1);
    }

    #[test]
    fn a_short_message_comes_whole_without_next_offset() {
        let v = message_body(&[msg("m1", Author::User, "ciao\nmondo", 0)], "m1", 0).unwrap();
        assert_eq!(v, json!({"message_id": "m1", "offset": 0, "total_chars": 10, "content": "ciao\nmondo"}));
        let empty = message_body(&[msg("m2", Author::User, "", 0)], "m2", 0).unwrap();
        assert_eq!((empty["content"].as_str(), empty.get("next_offset")), (Some(""), None));
    }

    #[test]
    fn an_unknown_message_or_an_offset_past_the_end_is_an_error() {
        let messages = [msg("m1", Author::User, "abc", 0)];
        assert!(message_body(&messages, "zzz", 0).unwrap_err().contains("not the id of a message"));
        assert!(message_body(&messages, "m1", 3).unwrap_err().contains("beyond the end"));
        assert!(message_body(&messages, "m1", 2).is_ok());
    }

    #[test]
    fn an_external_author_stays_untrusted_in_every_part() {
        let m = fixtures::external("m1", "gmail", "Anna", &"x".repeat(20_000));
        let messages = [m];
        assert_eq!(message_body(&messages, "m1", 0).unwrap()["untrusted"], true);
        assert_eq!(message_body(&messages, "m1", 10_000).unwrap()["untrusted"], true);
        let user = message_body(&[msg("m2", Author::User, "x", 0)], "m2", 0).unwrap();
        assert!(user.get("untrusted").is_none());
    }
}
