//! Le chiavi del connettore Gmail: configurazione, slot dei segreti, host e scope.

/// L'id del connettore.
pub const ID: &str = "gmail";
/// La chiave del campo di configurazione col client id OAuth.
pub const CLIENT_ID_KEY: &str = "client_id";
/// La chiave del campo di configurazione con l'intervallo fra due giri, in minuti.
pub const INTERVAL_KEY: &str = "interval_minutes";
/// Lo slot del segreto col client secret OAuth.
pub const CLIENT_SECRET_SLOT: &str = "client_secret";
/// Lo slot del segreto col refresh token.
pub const REFRESH_TOKEN_SLOT: &str = "refresh_token";
/// La chiave del filtro di una regola: la ricerca nella sintassi di Gmail.
pub const QUERY_KEY: &str = "query";

/// L'host della pagina di consenso.
pub const ACCOUNTS_HOST: &str = "accounts.google.com";
/// L'host dell'endpoint dei token.
pub const OAUTH_HOST: &str = "oauth2.googleapis.com";
/// L'host della Gmail API.
pub const API_HOST: &str = "gmail.googleapis.com";
/// Lo scope richiesto: lettura dei messaggi, nient'altro.
pub const SCOPE: &str = "https://www.googleapis.com/auth/gmail.readonly";

#[cfg(test)]
mod tests;
