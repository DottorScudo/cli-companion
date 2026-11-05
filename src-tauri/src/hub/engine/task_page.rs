//! Quale pezzo di un task legge `task_details` (puro, senza app): la scheda
//! intera oppure, con `field`, un campo lungo a partire da `offset`. I campi
//! si chiamano con il loro percorso nel JSON del task (`report.summary`); una
//! voce di un elenco troppo lunga per una pagina si legge da sola con il suo
//! indice (`report.follow_ups.3`, `report.tests.2.details`). L'output e' in
//! `output::task_details`.

/// Caratteri massimi del nome di un campo nei parametri del tool.
pub const FIELD_MAX: usize = 64;

/// I testi lunghi del task, che si leggono a pezzi (offset in caratteri).
pub const TEXT_FIELDS: [&str; 5] = ["task", "note", "question.text", "report.summary", "report.diffstat"];

/// Gli elenchi del task, che si leggono a pezzi (offset in voci).
pub const LIST_FIELDS: [&str; 9] = [
    "question.options",
    "report.findings",
    "report.follow_ups",
    "report.files_changed",
    "report.tests",
    "depends_on",
    "waiting_on",
    "cascade.dependents",
    "cascade.running",
];

/// L'elenco le cui voci sono oggetti, e i loro testi che si leggono a pezzi.
const OBJECT_LIST: &str = "report.tests";
pub const OBJECT_ITEM_PARTS: [&str; 2] = ["command", "details"];

/// Un pezzo di un campo del task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskPage {
    /// Il nome normalizzato del campo (vedi `parse_field`).
    pub field: String,
    /// Caratteri (testi) o voci (elenchi) gia' letti.
    pub offset: usize,
}

/// Il nome del campo chiesto, normalizzato; l'errore e' in inglese, per il modello.
pub fn parse_field(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if TEXT_FIELDS.contains(&raw) || LIST_FIELDS.contains(&raw) {
        return Ok(raw.to_string());
    }
    item_field(raw).ok_or_else(|| {
        format!(
            "`field` must be one of {}, or a single item named in `more` (like `report.follow_ups.3` or \
            `report.tests.2.details`)",
            TEXT_FIELDS.iter().chain(&LIST_FIELDS).map(|f| format!("`{f}`")).collect::<Vec<_>>().join(", ")
        )
    })
}

/// Una voce di un elenco: `<elenco>.<indice>`, e per le prove anche il testo (`.command`, `.details`).
fn item_field(raw: &str) -> Option<String> {
    let (list, rest) = LIST_FIELDS.iter().find_map(|list| Some((*list, raw.strip_prefix(list)?.strip_prefix('.')?)))?;
    let (index, part) = match rest.split_once('.') {
        Some((index, part)) => (index, Some(part)),
        None => (rest, None),
    };
    if index.is_empty() || !index.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let index: usize = index.parse().ok()?;
    match (list == OBJECT_LIST, part) {
        (false, None) => Some(item_name(list, index, None)),
        (true, Some(part)) if OBJECT_ITEM_PARTS.contains(&part) => Some(item_name(list, index, Some(part))),
        _ => None,
    }
}

/// Il nome della voce `index` dell'elenco `list` (o del suo testo `part`).
pub fn item_name(list: &str, index: usize, part: Option<&str>) -> String {
    match part {
        Some(part) => format!("{list}.{index}.{part}"),
        None => format!("{list}.{index}"),
    }
}

/// I campi di testo di una voce dell'elenco `list` che si leggono a pezzi;
/// vuoto se la voce e' essa stessa un testo.
pub fn item_parts(list: &str) -> &'static [&'static str] {
    if list == OBJECT_LIST {
        &OBJECT_ITEM_PARTS
    } else {
        &[]
    }
}

/// Il JSON pointer del campo nel JSON del task (`report.tests.2.details` -> `/report/tests/2/details`).
pub fn pointer(field: &str) -> String {
    field.split('.').fold(String::new(), |mut out, step| {
        out.push('/');
        out.push_str(step);
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_field_is_accepted_as_it_is() {
        for field in TEXT_FIELDS.iter().chain(&LIST_FIELDS) {
            assert_eq!(parse_field(&format!(" {field} ")).as_deref(), Ok(*field));
        }
    }

    #[test]
    fn a_single_item_is_named_by_its_index() {
        assert_eq!(parse_field("report.follow_ups.3").as_deref(), Ok("report.follow_ups.3"));
        assert_eq!(parse_field("depends_on.007").as_deref(), Ok("depends_on.7"));
        assert_eq!(parse_field("report.tests.2.details").as_deref(), Ok("report.tests.2.details"));
        assert_eq!(parse_field("report.tests.0.command").as_deref(), Ok("report.tests.0.command"));
    }

    #[test]
    fn unknown_fields_and_malformed_items_are_refused() {
        for raw in [
            "",
            "title",
            "status",
            "report",
            "report.commit",
            "session",
            "report.follow_ups.",
            "report.follow_ups.-1",
            "report.follow_ups.+1",
            "report.follow_ups.x",
            "report.follow_ups.1.text",
            "report.tests.1",
            "report.tests.1.result",
            "task.0",
            "report.follow_ups.99999999999999999999999",
        ] {
            let err = parse_field(raw).unwrap_err();
            assert!(err.contains("`field` must be one of") && err.contains("`report.summary`"), "{raw}: {err}");
        }
    }

    #[test]
    fn the_pointer_follows_the_dotted_path() {
        assert_eq!(pointer("task"), "/task");
        assert_eq!(pointer("report.tests.2.details"), "/report/tests/2/details");
    }

    #[test]
    fn only_the_tests_have_item_parts() {
        assert_eq!(item_parts("report.tests"), OBJECT_ITEM_PARTS);
        assert!(item_parts("report.follow_ups").is_empty());
    }
}
