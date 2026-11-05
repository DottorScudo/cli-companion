//! La sezione "Attachments" del task `plan` (pura, in inglese): i file allegati
//! alla richiesta dell'utente e ai messaggi recenti riportati nel testo, con
//! percorso assoluto, nome, tipo e dimensione. Il pianificatore li legge e ne
//! riporta i percorsi nelle specifiche dei task che propone, perche' i worker
//! partono dalla sola specifica. Mai il contenuto dei file.

use crate::hub::attachments::listing;
use crate::hub::model::{Message, Thread};
use std::collections::HashSet;
use std::path::Path;

const INSTRUCTION: &str = "The user attached these files to the messages above. You can read them at the absolute \
paths listed. The workers that will run the tasks you propose see only the specification of their task: write the \
paths of the files each task needs in its specification, together with what each file is.";

/// Le righe `percorso — nome (tipo, dimensione)` degli allegati di `request` e
/// dei messaggi `recent`, senza ripetere un messaggio presente in entrambi.
fn lines(root: &Path, thread: &Thread, request: Option<&Message>, recent: &[Message]) -> Vec<String> {
    let mut seen = HashSet::new();
    request
        .into_iter()
        .chain(recent)
        .filter(|m| seen.insert(m.id.as_str()))
        .flat_map(|m| &m.attachments)
        .map(|a| format!("- {}", listing::describe_with_path(root, &thread.channel, &thread.id, a)))
        .collect()
}

/// La sezione pronta da inserire nel testo, con la riga vuota finale; vuota
/// se nessuno di quei messaggi ha allegati.
pub fn section(root: &Path, thread: &Thread, request: Option<&Message>, recent: &[Message]) -> String {
    let lines = lines(root, thread, request, recent);
    if lines.is_empty() {
        return String::new();
    }
    format!("## Attachments\n\n{INSTRUCTION}\n\n{}\n\n", lines.join("\n"))
}
