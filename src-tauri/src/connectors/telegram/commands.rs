//! Comandi e invito di abbinamento nel testo di un messaggio (puri). Il connettore
//! riconosce la sintassi di Telegram (`/nome@bot argomenti`); li esegue il runtime.

use crate::connectors::contracts::ContactCommand;

/// Lunghezza massima del codice di un invito `/start <codice>`.
const PAYLOAD_MAX: usize = 64;

/// Cosa dice un messaggio di testo: un invito di abbinamento o un comando.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Parsed {
    pub pairing_code: Option<String>,
    pub command: Option<ContactCommand>,
}

/// `[A-Za-z0-9_-]{1,64}`: il codice di un invito (e di un link `?start=`).
pub fn valid_payload(payload: &str) -> bool {
    !payload.is_empty()
        && payload.len() <= PAYLOAD_MAX
        && payload.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn command_named(name: &str, args: &str) -> ContactCommand {
    let args = args.to_string();
    match name {
        "start" | "help" => ContactCommand::Help,
        "threads" => ContactCommand::Threads,
        "use" => ContactCommand::Use(args),
        "ws" => ContactCommand::Workspaces(args),
        "new" => ContactCommand::New(args),
        "status" => ContactCommand::Status,
        "mute" => ContactCommand::Mute(args),
        "unmute" => ContactCommand::Unmute,
        "unlink" | "stop" => ContactCommand::Unlink,
        _ => ContactCommand::Unknown,
    }
}

/// Il testo di un messaggio (non una didascalia). Senza `/` iniziale non e' un comando.
pub fn parse(text: &str) -> Parsed {
    let Some(rest) = text.strip_prefix('/') else { return Parsed::default() };
    let (head, args) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let name = head.split_once('@').map_or(head, |(name, _bot)| name).to_ascii_lowercase();
    let args = args.trim();
    if name == "start" && valid_payload(args) {
        return Parsed { pairing_code: Some(args.to_string()), command: None };
    }
    Parsed { pairing_code: None, command: Some(command_named(&name, args)) }
}

#[cfg(test)]
mod tests;
