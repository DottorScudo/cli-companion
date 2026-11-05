//! I messaggi di sistema del motore nei thread: in italiano (li legge
//! l'utente), senza dettagli tecnici ne' dati sensibili (mai la API key, mai
//! il corpo di una risposta dell'endpoint).

use super::audit::MAX_MUTATIONS_PER_TURN;
use super::board_ops::TaskOp;
use super::chat::ModelError;
use super::tools::ToolRejection;

const SETTINGS: &str = "Impostazioni › Agents HUB";

/// Menzione di un segretario senza system prompt.
pub fn not_configured(name: &str) -> String {
    format!("@{name} non è configurato: scrivi le sue istruzioni (system prompt) in {SETTINGS}.")
}

/// Menzione di un segretario fuori dai suoi canali.
pub fn not_allowed(name: &str) -> String {
    format!("@{name} non può essere menzionato in questo canale.")
}

/// Un messaggio senza menzioni che nessun segretario del thread puo' ricevere.
pub const UNANSWERED: &str = "Nessun segretario ha ricevuto questo messaggio: menzionane uno con @nome per svegliarlo.";

/// Il segretario della sessione e' stato tolto, svuotato o escluso dal canale.
pub fn secretary_gone(name: &str) -> String {
    format!("@{name} non è più disponibile in questo canale: la sua sessione è chiusa.")
}

/// Il turno si e' fermato al limite di passi.
pub fn step_limit(name: &str, steps: u32) -> String {
    format!(
        "@{name} si è fermato dopo {steps} passi senza una risposta finale. Riprova con una richiesta più semplice."
    )
}

/// Il modello ha chiesto un tool che il segretario non ha, o con argomenti sbagliati.
pub fn tool_rejected(name: &str, rejection: &ToolRejection) -> String {
    match rejection {
        ToolRejection::NotAllowed(tool) => {
            format!("@{name} ha chiesto il tool «{tool}», che non gli è consentito: richiesta rifiutata.")
        }
        ToolRejection::InvalidArguments { tool, .. } => {
            format!("@{name} ha chiamato «{}» con argomenti non validi: richiesta rifiutata.", tool.code())
        }
        ToolRejection::TurnLimit(tool) => format!(
            "@{name} ha chiesto «{}» dopo {MAX_MUTATIONS_PER_TURN} modifiche in questo turno, il massimo: richiesta \
            rifiutata.",
            tool.code()
        ),
    }
}

/// Il segretario ha consegnato al worker una risposta per conto dell'utente
/// (`relay_answer`): il thread mostra esattamente cosa e' stato inviato.
pub fn answer_relayed(name: &str, task_id: &str, body: &str) -> String {
    format!(
        "@{name} ha inviato al worker del task {task_id} questa risposta:
{body}"
    )
}

/// Il segretario ha ritentato o riordinato un task (`task_action`).
pub fn task_changed(name: &str, op: &TaskOp, title: &str, before_title: &str) -> String {
    match op {
        TaskOp::Retry => format!("@{name} ha ritentato «{title}»."),
        TaskOp::ToTop => format!("@{name} ha portato «{title}» in testa alla coda."),
        TaskOp::MoveBefore { .. } => format!("@{name} ha spostato «{title}» prima di «{before_title}»."),
    }
}

/// Il segretario ha cambiato pausa e/o parallelismo della board del
/// workspace (`board_control`); `None` se non e' cambiato nulla.
pub fn board_changed(name: &str, workspace: &str, paused: Option<bool>, max_parallel: Option<u64>) -> Option<String> {
    let pause = |paused: bool| if paused { "messo in pausa" } else { "ripreso" };
    let parallel = |n: u64| format!("impostato a {n} i task in parallelo");
    let what = match (paused, max_parallel) {
        (None, None) => return None,
        (Some(p), None) => format!("{} la board di {workspace}", pause(p)),
        (None, Some(n)) => format!("{} della board di {workspace}", parallel(n)),
        (Some(p), Some(n)) => format!("{} la board di {workspace} e {}", pause(p), parallel(n)),
    };
    Some(format!("@{name} ha {what}."))
}

/// La chiamata al modello e' fallita.
pub fn model_error(name: &str, error: ModelError) -> String {
    let reason = match error {
        ModelError::NotConfigured => format!("indirizzo e modello del provider non sono impostati ({SETTINGS})"),
        ModelError::KeyUnavailable => "la API key non si legge dal portachiavi del sistema".to_string(),
        ModelError::Unreachable => "il modello non è raggiungibile: controlla l'indirizzo del provider".to_string(),
        ModelError::Timeout => "il modello non ha risposto in tempo".to_string(),
        ModelError::Http(401 | 403) => "il provider ha rifiutato la richiesta: controlla la API key".to_string(),
        ModelError::Http(404) => {
            "il provider non trova l'indirizzo o il modello: controlla le impostazioni".to_string()
        }
        ModelError::Http(429) => "il provider ha troppe richieste: riprova tra poco".to_string(),
        ModelError::Http(code) => format!("il provider ha risposto con un errore (HTTP {code})"),
        ModelError::BadResponse => "la risposta del modello non è valida".to_string(),
        ModelError::Empty => "il modello ha restituito una risposta vuota".to_string(),
    };
    format!("@{name} non può rispondere: {reason}.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::settings::SecretaryTool;

    #[test]
    fn every_model_error_has_its_own_short_text() {
        let errors = [
            ModelError::NotConfigured,
            ModelError::KeyUnavailable,
            ModelError::Unreachable,
            ModelError::Timeout,
            ModelError::Http(401),
            ModelError::Http(404),
            ModelError::Http(429),
            ModelError::Http(500),
            ModelError::BadResponse,
            ModelError::Empty,
        ];
        let texts: Vec<String> = errors.iter().map(|e| model_error("mario", *e)).collect();
        for (i, t) in texts.iter().enumerate() {
            assert!(t.starts_with("@mario non può rispondere: ") && t.ends_with('.'), "{t}");
            assert!(texts.iter().skip(i + 1).all(|other| other != t), "{t}");
        }
        assert!(model_error("mario", ModelError::Http(502)).contains("HTTP 502"));
    }

    #[test]
    fn rejections_name_the_tool() {
        let not_allowed = tool_rejected("mario", &ToolRejection::NotAllowed("open_plan_task".into()));
        assert!(not_allowed.contains("«open_plan_task»"));
        let invalid = ToolRejection::InvalidArguments { tool: SecretaryTool::PostMessage, reason: "x".into() };
        assert!(tool_rejected("mario", &invalid).contains("«post_message» con argomenti non validi"));
        assert_eq!(
            tool_rejected("mario", &ToolRejection::TurnLimit(SecretaryTool::OpenPlanTask)),
            "@mario ha chiesto «open_plan_task» dopo 3 modifiche in questo turno, il massimo: richiesta rifiutata."
        );
        assert!(step_limit("mario", 8).contains("dopo 8 passi"));
        assert_eq!(
            answer_relayed("mario", "t-7", "Usa Postgres."),
            "@mario ha inviato al worker del task t-7 questa risposta:
Usa Postgres."
        );
    }

    #[test]
    fn board_changes_say_what_changed_by_title_and_workspace_name() {
        let before = TaskOp::MoveBefore { before_task_id: "t-2".into() };
        assert_eq!(task_changed("ugo", &TaskOp::Retry, "Login", ""), "@ugo ha ritentato «Login».");
        assert_eq!(task_changed("ugo", &TaskOp::ToTop, "Login", ""), "@ugo ha portato «Login» in testa alla coda.");
        assert_eq!(task_changed("ugo", &before, "Login", "Logout"), "@ugo ha spostato «Login» prima di «Logout».");
        let board = |paused, max| board_changed("ugo", "Sito Web", paused, max);
        assert_eq!(board(Some(true), None).unwrap(), "@ugo ha messo in pausa la board di Sito Web.");
        assert_eq!(board(Some(false), None).unwrap(), "@ugo ha ripreso la board di Sito Web.");
        assert_eq!(board(None, Some(2)).unwrap(), "@ugo ha impostato a 2 i task in parallelo della board di Sito Web.");
        assert_eq!(
            board(Some(true), Some(1)).unwrap(),
            "@ugo ha messo in pausa la board di Sito Web e impostato a 1 i task in parallelo."
        );
        assert_eq!(board(None, None), None);
    }
}
