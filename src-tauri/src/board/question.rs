//! Domanda bloccante di un worker all'utente (tool `ask_user`): il tipo che si
//! salva sul task e la validazione pura dei parametri. La domanda la scrive solo
//! `lifecycle`; qui non c'e' stato, solo forma e limiti.

use crate::validate::req_text;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// Lunghezza massima del testo di una domanda (in caratteri, dopo il trim).
pub const QUESTION_MAX: usize = 2000;
/// Numero massimo di opzioni suggerite.
pub const OPTIONS_MAX: usize = 5;
/// Lunghezza massima di un'opzione (in caratteri, su una riga).
pub const OPTION_MAX: usize = 80;
/// Lunghezza massima della risposta tenuta e digitata nel terminale (in caratteri).
pub const ANSWER_MAX: usize = 4000;
/// Domande tenute per task: oltre, le piu' vecchie si scartano.
pub const QUESTIONS_KEPT: usize = 20;

/// Una domanda del worker all'utente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TaskQuestion {
    /// `q-<tentativo>-<n>`, con `n` la posizione (da 1) fra le domande di quel tentativo.
    pub id: String,
    /// Tentativo (`attempts` del task) in cui il worker l'ha fatta.
    pub attempt: u32,
    pub text: String,
    /// Risposte suggerite dal worker; l'utente puo' rispondere anche con testo libero.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub options: Vec<String>,
    pub asked_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub answered_at: Option<i64>,
    /// Testo della risposta consegnata dall'app; assente con `answered_at` = l'utente ha risposto direttamente nel terminale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub answer: Option<String>,
}

/// Valida i parametri di `ask_user`: ritorna il testo della domanda e le opzioni
/// (trim fatto). Gli errori li legge il modello: in inglese.
pub fn parse_question(params: &Value) -> Result<(String, Vec<String>), String> {
    let text = match params.get("question") {
        Some(Value::String(s)) => req_text("question", s, QUESTION_MAX)?,
        None | Some(Value::Null) => return Err("question is missing".to_string()),
        Some(_) => return Err("question must be a string".to_string()),
    };
    let raw = match params.get("options") {
        None | Some(Value::Null) => return Ok((text, Vec::new())),
        Some(Value::Array(items)) => items,
        Some(_) => return Err("options must be an array of strings".to_string()),
    };
    if raw.len() > OPTIONS_MAX {
        return Err(format!("too many options (max {OPTIONS_MAX})"));
    }
    let mut options: Vec<String> = Vec::with_capacity(raw.len());
    for item in raw {
        let Value::String(s) = item else {
            return Err("options must be an array of strings".to_string());
        };
        let option = req_text("option", s, OPTION_MAX)?;
        if option.contains(['\n', '\r']) {
            return Err("an option must be a single line".to_string());
        }
        if options.contains(&option) {
            return Err(format!("duplicate option: {option}"));
        }
        options.push(option);
    }
    Ok((text, options))
}

#[cfg(test)]
mod tests;
