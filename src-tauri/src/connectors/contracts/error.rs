//! La classificazione degli errori di un connettore: il supervisore decide da qui
//! backoff, fermo e stato di salute (docs/CONNECTORS.MD §7). I messaggi sono gia'
//! ripuliti dal connettore: mai segreti, URL con token, corpi di risposta o
//! contenuti degli elementi.

use std::fmt;
use std::time::Duration;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorError {
    /// Rete, timeout, 5xx: si riprova con il backoff.
    Transient(String),
    /// 429: si rispetta `retry_after` (con i tetti del runtime).
    RateLimited { retry_after: Option<Duration> },
    /// 401/403: il supervisore si ferma in `needs_auth` finche' l'utente non interviene.
    Auth(String),
    /// Configurazione non valida: si ferma in `error` fino a una modifica dell'istanza.
    Config(String),
    /// Errore non recuperabile (o un permesso non dichiarato): si ferma in `error`.
    Fatal(String),
}

impl ConnectorError {
    /// La classe dell'errore, per il log e per la salute: mai il messaggio.
    pub fn class(&self) -> &'static str {
        match self {
            Self::Transient(_) => "transient",
            Self::RateLimited { .. } => "rate_limited",
            Self::Auth(_) => "auth",
            Self::Config(_) => "config",
            Self::Fatal(_) => "fatal",
        }
    }

    /// Se il supervisore deve fermarsi invece di riprovare.
    pub fn stops_the_supervisor(&self) -> bool {
        matches!(self, Self::Auth(_) | Self::Config(_) | Self::Fatal(_))
    }

    /// Il messaggio generico, in italiano, per `last_error` e per l'interfaccia:
    /// dipende solo dalla classe, mai dal testo del connettore.
    pub fn public_message(&self) -> &'static str {
        match self {
            Self::Transient(_) => "Servizio non raggiungibile: nuovo tentativo a breve.",
            Self::RateLimited { .. } => "Il servizio ha limitato le richieste: nuovo tentativo a breve.",
            Self::Auth(_) => "Credenziali rifiutate: serve il tuo intervento.",
            Self::Config(_) => "Configurazione non valida.",
            Self::Fatal(_) => "Errore non recuperabile.",
        }
    }
}

impl fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transient(m) | Self::Auth(m) | Self::Config(m) | Self::Fatal(m) => {
                write!(f, "{}: {m}", self.class())
            }
            Self::RateLimited { retry_after: Some(after) } => write!(f, "rate_limited: attesa {}s", after.as_secs()),
            Self::RateLimited { retry_after: None } => write!(f, "rate_limited"),
        }
    }
}

impl std::error::Error for ConnectorError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_transient_and_rate_limited_errors_are_retried() {
        assert!(!ConnectorError::Transient("x".into()).stops_the_supervisor());
        assert!(!ConnectorError::RateLimited { retry_after: None }.stops_the_supervisor());
        for e in
            [ConnectorError::Auth("x".into()), ConnectorError::Config("x".into()), ConnectorError::Fatal("x".into())]
        {
            assert!(e.stops_the_supervisor(), "{e}");
        }
    }

    #[test]
    fn the_public_message_depends_only_on_the_class() {
        let a = ConnectorError::Transient("https://api.example.com/bot123:TOKEN/getUpdates".into());
        let b = ConnectorError::Transient("altro".into());
        assert_eq!(a.public_message(), b.public_message());
        assert!(!a.public_message().contains("TOKEN"));
        assert_eq!(a.class(), "transient");
    }
}
