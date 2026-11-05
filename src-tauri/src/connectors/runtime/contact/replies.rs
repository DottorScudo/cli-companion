//! Tutti i testi che il bot manda all'utente (aiuto, elenchi, conferme, errori),
//! in italiano e senza markup. Puri: niente HUB ne' connettore, cosi' si provano
//! a parole. I nomi di file e di thread che arrivano da fuori si ripuliscono qui.

use crate::hub::attachments::{ATTACHMENTS_MAX, ATTACHMENT_SIZE_MAX};

/// Caratteri al massimo di un nome di file ripetuto all'utente.
const FILE_NAME_SHOWN_MAX: usize = 40;
const BYTES_PER_MB: u64 = 1024 * 1024;
/// Il segnaposto davanti a un thread che aspetta l'utente.
const WAITING_MARKER: &str = "⏳ ";
/// Il segnaposto davanti al canale scelto con `/ws`.
const CHOSEN_MARKER: &str = "✅ ";

/// Una riga dell'elenco dei thread.
pub struct ThreadLine<'a> {
    pub label: &'a str,
    pub waiting: bool,
}

/// Una riga dell'elenco dei canali.
pub struct ChannelLine<'a> {
    pub name: &'a str,
    pub chosen: bool,
}

/// Se le notifiche sono silenziate, e fino a quando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuteState {
    Off,
    Forever,
    /// Minuti che mancano (almeno uno).
    Minutes(u64),
}

/// Perche' un allegato non e' stato importato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    TooBig,
    TooMany,
    NotDownloaded,
}

/// Un allegato non importato, col nome mostrato dal servizio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedFile {
    pub name: String,
    pub reason: SkipReason,
}

pub fn thread_label(channel_name: &str, title: &str) -> String {
    format!("{channel_name} › {title}")
}

pub fn help() -> String {
    [
        "Comandi:",
        "/threads: i thread aperti",
        "/use <n>: scegli il thread dall'ultimo elenco",
        "/ws [n|nome]: scegli il workspace dove /new apre i thread",
        "/new <titolo>: apri un thread nuovo",
        "/status: thread corrente e cosa aspetta te",
        "/mute [minuti]: silenzia le notifiche",
        "/unmute: riattiva le notifiche",
        "/unlink: scollega questo account",
        "/help: questo elenco",
        "",
        "Un messaggio normale va nel thread corrente. Se rispondi a una notifica, scrivi nel thread di quella notifica.",
    ]
    .join("\n")
}

pub fn unknown_command() -> String {
    format!("Comando sconosciuto.\n\n{}", help())
}

pub fn rate_limited() -> String {
    "Troppi messaggi: riprova tra un minuto.".to_string()
}

pub fn hub_error(error: &str) -> String {
    error.to_string()
}

pub fn not_saved() -> String {
    "Non sono riuscito a salvare la scelta: riprova tra poco.".to_string()
}

pub fn no_threads() -> String {
    "Non ci sono thread aperti. Creane uno con /new <titolo>.".to_string()
}

pub fn threads_list(lines: &[ThreadLine<'_>]) -> String {
    let rows: Vec<String> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let marker = if line.waiting { WAITING_MARKER } else { "" };
            format!("{}. {marker}{}", i + 1, line.label)
        })
        .collect();
    format!("{}\n\nScegli con /use <n>.", rows.join("\n"))
}

pub fn use_usage(listed: usize) -> String {
    if listed == 0 {
        "Prima elenca i thread con /threads, poi scegli con /use <n>.".to_string()
    } else {
        format!("Uso: /use <n>, con n da 1 a {listed} (dall'ultimo elenco di /threads).")
    }
}

pub fn thread_gone() -> String {
    "Quel thread non esiste più: rifai /threads.".to_string()
}

pub fn now_using(label: &str) -> String {
    format!("Thread corrente: {label}")
}

pub fn workspaces_list(lines: &[ChannelLine<'_>]) -> String {
    let rows: Vec<String> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let marker = if line.chosen { CHOSEN_MARKER } else { "" };
            format!("{}. {marker}{}", i + 1, line.name)
        })
        .collect();
    format!("{}\n\nScegli con /ws <n> o /ws <nome>: /new apre i thread li'.", rows.join("\n"))
}

pub fn workspace_chosen(name: &str) -> String {
    format!("Workspace scelto: {name}. I thread nuovi si aprono li'.")
}

pub fn workspace_not_found() -> String {
    "Workspace non trovato: guarda l'elenco con /ws.".to_string()
}

pub fn new_usage() -> String {
    "Uso: /new <titolo>".to_string()
}

pub fn thread_created(title: &str) -> String {
    format!("Thread creato e scelto: {title}")
}

pub fn new_failed(error: &str) -> String {
    format!("Non ho creato il thread: {error}")
}

/// `waiting_by_channel`: i thread che aspettano l'utente per nome di canale.
pub fn status(current: &str, waiting_by_channel: &[(String, usize)], mute: MuteState) -> String {
    let total: usize = waiting_by_channel.iter().map(|(_, n)| n).sum();
    let waiting = if total == 0 {
        "Nessun thread aspetta te.".to_string()
    } else {
        let per_channel: Vec<String> = waiting_by_channel.iter().map(|(name, n)| format!("{name}: {n}")).collect();
        format!("Thread che aspettano te: {total} ({}).", per_channel.join(", "))
    };
    let mute = match mute {
        MuteState::Off => "Notifiche attive.".to_string(),
        MuteState::Forever => "Notifiche silenziate fino a /unmute.".to_string(),
        MuteState::Minutes(minutes) => format!("Notifiche silenziate ancora per {minutes} min."),
    };
    format!("Thread corrente: {current}\n{waiting}\n{mute}")
}

pub fn no_current_label() -> String {
    "nessuno".to_string()
}

pub fn current_no_longer_open() -> String {
    "non più aperto".to_string()
}

pub fn muted_forever() -> String {
    "Notifiche silenziate fino a /unmute.".to_string()
}

pub fn muted_for(minutes: u64) -> String {
    format!("Notifiche silenziate per {minutes} min. Riattivale con /unmute.")
}

pub fn mute_usage(max_minutes: u64) -> String {
    format!("Uso: /mute [minuti], con minuti da 1 a {max_minutes}. Senza numero silenzio finché non scrivi /unmute.")
}

pub fn unmuted() -> String {
    "Notifiche riattivate.".to_string()
}

pub fn unlinked() -> String {
    "Account scollegato: non riceverai più notifiche e non scrivo più nei thread. Per ricollegarti serve un nuovo invito."
        .to_string()
}

pub fn no_current_thread() -> String {
    "Non hai un thread corrente: elenca i thread con /threads e scegli con /use <n>, oppure apri un thread con /new <titolo>."
        .to_string()
}

pub fn current_thread_gone() -> String {
    "Il thread corrente non esiste più: scegline un altro con /threads e /use <n>, oppure apri un thread con /new <titolo>."
        .to_string()
}

pub fn stale_reply() -> String {
    "Il messaggio a cui rispondi non è più collegato: l'ho scritto nel thread corrente.".to_string()
}

pub fn not_delivered(error: &str) -> String {
    format!("Messaggio non consegnato: {error}")
}

fn skipped_reason(reason: SkipReason) -> &'static str {
    match reason {
        SkipReason::TooBig => "troppo grande",
        SkipReason::TooMany => "troppi allegati",
        SkipReason::NotDownloaded => "download non riuscito",
    }
}

fn shown_file_name(name: &str) -> String {
    let clean: String = name.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let clean = clean.trim();
    if clean.chars().count() > FILE_NAME_SHOWN_MAX {
        let cut: String = clean.chars().take(FILE_NAME_SHOWN_MAX).collect();
        format!("{cut}…")
    } else {
        clean.to_string()
    }
}

pub fn attachments_skipped(skipped: &[SkippedFile]) -> String {
    let list: Vec<String> =
        skipped.iter().map(|f| format!("{} ({})", shown_file_name(&f.name), skipped_reason(f.reason))).collect();
    format!(
        "Allegati non importati: {}. Limiti: {} MB per file, {ATTACHMENTS_MAX} per messaggio.",
        list.join(", "),
        ATTACHMENT_SIZE_MAX / BYTES_PER_MB
    )
}

#[cfg(test)]
mod tests;
