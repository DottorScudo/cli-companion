//! Il client HTTP del contesto di un connettore: l'unico modo in cui un connettore
//! parla con la rete. Costruito sul `reqwest::Client` condiviso, rifiuta ogni
//! richiesta verso un host non dichiarato in `permissions.hosts` (confronto esatto
//! dell'host dopo il parsing dell'URL, solo `https`, niente credenziali nell'URL,
//! solo la porta predefinita), segue i redirect a mano e solo verso host
//! dichiarati, ha un timeout per chiamata e un tetto alla dimensione della
//! risposta. Gli errori sono ripuliti: non contengono mai l'URL (il token del bot
//! Telegram sta nell'URL), la richiesta o la risposta.
//!
//! Le decisioni (`check_url`, `redirect_target`, `redirect_request`,
//! `append_capped`) sono funzioni pure, provate senza rete.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "l'elenco degli host del client HTTP non e' ancora letto da nessun connettore registrato"
    )
)]

use crate::connectors::contracts::ConnectorError;
use crate::connectors::limits::{CALL_TIMEOUT, REDIRECTS_MAX, RESPONSE_MAX, ROUND_TIMEOUT};
use reqwest::{Client, Url};
use std::fmt;
use std::time::{Duration, Instant};

/// Tempo massimo per stabilire la connessione.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const USER_AGENT: &str = concat!("cli-companion/", env!("CARGO_PKG_VERSION"));

/// Perche' una richiesta non e' andata a buon fine. Nessuna variante porta testo
/// dalla rete: il messaggio dipende solo dalla classe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpError {
    /// Schema, host, porta o credenziali dell'URL non consentiti dal descrittore.
    Blocked,
    /// L'URL o la richiesta non sono costruibili.
    BadRequest,
    Timeout,
    Network,
    /// La risposta supera il tetto.
    TooLarge,
    TooManyRedirects,
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Blocked => "richiesta verso un indirizzo non dichiarato dal connettore",
            Self::BadRequest => "richiesta non valida",
            Self::Timeout => "tempo scaduto",
            Self::Network => "errore di rete",
            Self::TooLarge => "risposta troppo grande",
            Self::TooManyRedirects => "troppi reindirizzamenti",
        })
    }
}

impl std::error::Error for HttpError {}

impl From<HttpError> for ConnectorError {
    /// Una richiesta bloccata o malformata e' un errore del connettore (si ferma);
    /// gli altri sono del servizio o della rete (si riprova).
    fn from(error: HttpError) -> Self {
        match error {
            HttpError::Blocked | HttpError::BadRequest => ConnectorError::Fatal(error.to_string()),
            _ => ConnectorError::Transient(error.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

/// Una richiesta. Le intestazioni sono quelle del connettore (es. `Authorization`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    /// Tempo totale, redirect compresi (al piu' `ROUND_TIMEOUT`).
    pub timeout: Duration,
}

impl HttpRequest {
    pub fn get(url: &str) -> Self {
        Self { method: HttpMethod::Get, url: url.to_string(), headers: Vec::new(), body: None, timeout: CALL_TIMEOUT }
    }

    pub fn post(url: &str, body: Vec<u8>) -> Self {
        Self { method: HttpMethod::Post, body: Some(body), ..Self::get(url) }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// La risposta intera (entro il tetto). Le intestazioni hanno il nome in minuscolo
/// e solo i valori testuali.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// L'URL e' ammesso per `hosts`? `https`, nessuna credenziale, porta predefinita
/// e host (un nome, non un indirizzo IP) uguale a uno dichiarato. Il parser
/// normalizza maiuscole e forma dell'host: `https://a.com@evil.com/` ha host `evil.com`.
pub fn check_url(url: &Url, hosts: &[&str]) -> Result<(), HttpError> {
    let host_ok = url.domain().is_some_and(|domain| hosts.contains(&domain));
    let plain = url.username().is_empty() && url.password().is_none() && url.port().is_none();
    if url.scheme() == "https" && host_ok && plain {
        Ok(())
    } else {
        Err(HttpError::Blocked)
    }
}

/// Dove porta un redirect `Location` partendo da `current`, dopo `hops_done`
/// redirect gia' seguiti: lo stesso controllo di `check_url` e il tetto dei salti.
pub fn redirect_target(current: &Url, location: &str, hosts: &[&str], hops_done: usize) -> Result<Url, HttpError> {
    if hops_done >= REDIRECTS_MAX {
        return Err(HttpError::TooManyRedirects);
    }
    let next = current.join(location).map_err(|_| HttpError::BadRequest)?;
    check_url(&next, hosts)?;
    Ok(next)
}

/// Come prosegue la richiesta dopo un redirect `status`: il metodo e se il corpo
/// si rimanda (303, e 301/302 su POST, diventano GET senza corpo).
pub fn redirect_request(method: HttpMethod, status: u16) -> (HttpMethod, bool) {
    match (status, method) {
        (303, _) | (301 | 302, HttpMethod::Post) => (HttpMethod::Get, false),
        _ => (method, true),
    }
}

/// Aggiunge `chunk` a `buf` se resta entro `max`.
fn append_capped(buf: &mut Vec<u8>, chunk: &[u8], max: usize) -> Result<(), HttpError> {
    if buf.len().saturating_add(chunk.len()) > max {
        return Err(HttpError::TooLarge);
    }
    buf.extend_from_slice(chunk);
    Ok(())
}

/// Classifica un errore di reqwest senza riportarne il testo (include l'URL).
fn classify(error: &reqwest::Error) -> HttpError {
    if error.is_timeout() {
        HttpError::Timeout
    } else if error.is_builder() {
        HttpError::BadRequest
    } else {
        HttpError::Network
    }
}

/// reqwest e' compilato senza provider crittografico (come per l'updater): si
/// installa `ring` se nessuno l'ha ancora fatto. Se un'installazione concorrente
/// vince, il provider c'e' comunque: l'esito si puo' ignorare.
fn ensure_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
}

/// Il client condiviso da tutti i connettori: niente redirect automatici (li
/// segue `ContextHttp` controllandoli), solo `https`.
pub fn shared_client() -> Result<Client, String> {
    ensure_crypto_provider();
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .https_only(true)
        .connect_timeout(CONNECT_TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|_| "client HTTP dei connettori non creato".to_string())
}

/// Il client HTTP di un connettore: il client condiviso con gli host del suo descrittore.
#[derive(Clone)]
pub struct ContextHttp {
    client: Client,
    hosts: &'static [&'static str],
    max_body: usize,
}

impl ContextHttp {
    pub fn new(client: Client, hosts: &'static [&'static str]) -> Self {
        Self { client, hosts, max_body: RESPONSE_MAX }
    }

    /// Alza il tetto della risposta (per scaricare un allegato).
    pub fn with_max_body(mut self, max_body: usize) -> Self {
        self.max_body = max_body;
        self
    }

    pub fn hosts(&self) -> &'static [&'static str] {
        self.hosts
    }

    pub async fn get(&self, url: &str) -> Result<HttpResponse, HttpError> {
        self.send(HttpRequest::get(url)).await
    }

    /// Esegue la richiesta: ogni URL (anche quelli dei redirect) passa da `check_url`.
    /// Le intestazioni del connettore non vanno oltre un redirect verso un altro host.
    pub async fn send(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        let mut url = Url::parse(&request.url).map_err(|_| HttpError::BadRequest)?;
        check_url(&url, self.hosts)?;
        let deadline = Instant::now() + request.timeout.min(ROUND_TIMEOUT);
        let (mut method, mut body, mut headers) = (request.method, request.body, request.headers);
        for hops in 0.. {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(HttpError::Timeout);
            }
            let mut builder = match method {
                HttpMethod::Get => self.client.get(url.clone()),
                HttpMethod::Post => self.client.post(url.clone()),
            };
            builder = builder.timeout(left);
            for (name, value) in &headers {
                builder = builder.header(name.as_str(), value.as_str());
            }
            if let Some(bytes) = &body {
                builder = builder.body(bytes.clone());
            }
            let mut response = builder.send().await.map_err(|e| classify(&e))?;
            let status = response.status();
            if status.is_redirection() {
                let location = response.headers().get(reqwest::header::LOCATION).and_then(|v| v.to_str().ok());
                if let Some(location) = location {
                    let next = redirect_target(&url, location, self.hosts, hops)?;
                    if next.host_str() != url.host_str() {
                        headers.clear();
                    }
                    let (next_method, keep_body) = redirect_request(method, status.as_u16());
                    method = next_method;
                    if !keep_body {
                        body = None;
                    }
                    url = next;
                    continue;
                }
            }
            let response_headers = response
                .headers()
                .iter()
                .filter_map(|(n, v)| Some((n.as_str().to_string(), v.to_str().ok()?.to_string())))
                .collect();
            let mut data = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|e| classify(&e))? {
                append_capped(&mut data, &chunk, self.max_body)?;
            }
            return Ok(HttpResponse { status: status.as_u16(), headers: response_headers, body: data });
        }
        Err(HttpError::TooManyRedirects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOSTS: &[&str] = &["api.example.com", "files.example.com"];

    fn url(raw: &str) -> Url {
        Url::parse(raw).unwrap()
    }

    fn allowed(raw: &str) -> bool {
        check_url(&url(raw), HOSTS).is_ok()
    }

    #[test]
    fn declared_https_hosts_are_allowed() {
        assert!(allowed("https://api.example.com/v1/x?y=1#z"));
        assert!(allowed("https://files.example.com"));
        assert!(allowed("https://API.Example.COM/path"), "il parser porta l'host in minuscolo");
        assert!(allowed("https://api.example.com:443/"), "la porta predefinita sparisce nel parsing");
    }

    #[test]
    fn undeclared_hosts_are_rejected() {
        for raw in [
            "https://evil.com/",
            "https://example.com/",
            "https://sub.api.example.com/",
            "https://api.example.com.evil.com/",
            "https://evilapi.example.com/",
            "https://api.example.com@evil.com/",
            "https://evil.com\\@api.example.com/",
            "https://evil.com#@api.example.com/",
            "https://127.0.0.1/",
            "https://[::1]/",
            "https://localhost/",
        ] {
            assert_eq!(check_url(&url(raw), HOSTS), Err(HttpError::Blocked), "{raw}");
        }
    }

    #[test]
    fn only_plain_https_is_allowed() {
        for raw in [
            "http://api.example.com/",
            "ftp://api.example.com/",
            "wss://api.example.com/",
            "file:///etc/passwd",
            "https://user@api.example.com/",
            "https://user:pass@api.example.com/",
            "https://api.example.com:8443/",
        ] {
            assert_eq!(check_url(&url(raw), HOSTS), Err(HttpError::Blocked), "{raw}");
        }
        assert_eq!(check_url(&url("https://api.example.com/"), &[]), Err(HttpError::Blocked), "nessun host dichiarato");
    }

    #[test]
    fn redirects_are_followed_only_to_declared_https_hosts() {
        let current = url("https://api.example.com/a/b");
        let follow = |location: &str, hops| redirect_target(&current, location, HOSTS, hops).map(|u| u.to_string());
        assert_eq!(follow("/c", 0).unwrap(), "https://api.example.com/c");
        assert_eq!(follow("d?x=1", 2).unwrap(), "https://api.example.com/a/d?x=1");
        assert_eq!(follow("https://files.example.com/f", 0).unwrap(), "https://files.example.com/f");
        assert_eq!(follow("https://evil.com/f", 0), Err(HttpError::Blocked));
        assert_eq!(follow("//evil.com/f", 0), Err(HttpError::Blocked), "URL relativo al protocollo");
        assert_eq!(follow("http://api.example.com/f", 0), Err(HttpError::Blocked), "declassamento a http");
        assert_eq!(follow("https://api.example.com@evil.com/", 0), Err(HttpError::Blocked));
        assert_eq!(follow("http://[", 0), Err(HttpError::BadRequest));
        assert_eq!(follow("/c", REDIRECTS_MAX), Err(HttpError::TooManyRedirects));
        assert!(follow("/c", REDIRECTS_MAX - 1).is_ok());
    }

    #[test]
    fn redirect_status_decides_method_and_body() {
        assert_eq!(redirect_request(HttpMethod::Post, 303), (HttpMethod::Get, false));
        assert_eq!(redirect_request(HttpMethod::Post, 302), (HttpMethod::Get, false));
        assert_eq!(redirect_request(HttpMethod::Post, 301), (HttpMethod::Get, false));
        assert_eq!(redirect_request(HttpMethod::Get, 303), (HttpMethod::Get, false));
        assert_eq!(redirect_request(HttpMethod::Post, 307), (HttpMethod::Post, true));
        assert_eq!(redirect_request(HttpMethod::Post, 308), (HttpMethod::Post, true));
        assert_eq!(redirect_request(HttpMethod::Get, 302), (HttpMethod::Get, true));
    }

    #[test]
    fn the_response_cap_is_enforced_chunk_by_chunk() {
        let mut buf = Vec::new();
        assert!(append_capped(&mut buf, b"abcd", 6).is_ok());
        assert_eq!(append_capped(&mut buf, b"efg", 6), Err(HttpError::TooLarge));
        assert_eq!(buf, b"abcd", "il chunk che sfora non entra");
        assert!(append_capped(&mut buf, b"ef", 6).is_ok());
        assert_eq!(append_capped(&mut buf, &[0; 1], usize::MAX).map(|()| buf.len()), Ok(7));
        assert_eq!(append_capped(&mut buf, b"x", 7), Err(HttpError::TooLarge));
    }

    #[test]
    fn errors_are_generic_and_map_to_connector_classes() {
        for error in [
            HttpError::Blocked,
            HttpError::BadRequest,
            HttpError::Timeout,
            HttpError::Network,
            HttpError::TooLarge,
            HttpError::TooManyRedirects,
        ] {
            let text = error.to_string();
            assert!(!text.contains("http") && !text.contains('/'), "{text}");
        }
        assert!(matches!(ConnectorError::from(HttpError::Blocked), ConnectorError::Fatal(_)));
        assert!(matches!(ConnectorError::from(HttpError::BadRequest), ConnectorError::Fatal(_)));
        assert!(matches!(ConnectorError::from(HttpError::Timeout), ConnectorError::Transient(_)));
        assert!(matches!(ConnectorError::from(HttpError::Network), ConnectorError::Transient(_)));
    }

    fn context() -> ContextHttp {
        ContextHttp::new(shared_client().unwrap(), &["api.example.com"])
    }

    #[test]
    fn send_refuses_undeclared_hosts_and_schemes_before_any_io() {
        let http = context();
        let secret_url = "https://evil.example.org/bot123:SEGRETO/getMe";
        for raw in [secret_url, "http://api.example.com/x", "https://api.example.com:9/x", "https://127.0.0.1/"] {
            let result = tauri::async_runtime::block_on(http.get(raw));
            assert_eq!(result, Err(HttpError::Blocked), "{raw}");
        }
        let result = tauri::async_runtime::block_on(http.get("non un url"));
        assert_eq!(result, Err(HttpError::BadRequest));
        let message = tauri::async_runtime::block_on(http.get(secret_url)).unwrap_err().to_string();
        assert!(!message.contains("SEGRETO") && !message.contains("evil"), "{message}");
    }

    #[test]
    fn the_response_helpers_work() {
        let response = HttpResponse {
            status: 204,
            headers: vec![("retry-after".to_string(), "30".to_string())],
            body: Vec::new(),
        };
        assert_eq!(response.header("Retry-After"), Some("30"));
        assert!(response.is_success() && !HttpResponse { status: 429, ..response }.is_success());
        let request =
            HttpRequest::post("https://api.example.com/x", vec![1]).header("a", "b").timeout(Duration::from_secs(5));
        assert_eq!((request.method, request.headers.len(), request.timeout.as_secs()), (HttpMethod::Post, 1, 5));
    }
}
