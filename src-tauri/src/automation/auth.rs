//! Segreti per sessione e autenticazione delle richieste RPC e degli hook.
//!
//! Ogni terminale riceve un segreto casuale suo (in `CLI_COMPANION_TOKEN`),
//! valido finche' vive il flusso della sua shell: un processo di un terminale
//! non puo' spacciarsi per un'altra sessione, perche' non ne conosce il
//! segreto. Il segreto non finisce mai nei log ne' in un `Debug`.

use super::*;

/// Byte casuali di un segreto di sessione (256 bit).
const SECRET_BYTES: usize = 32;
/// Byte casuali del suffisso dell'id di una sessione di prova.
const PROBE_SUFFIX_BYTES: usize = 8;
/// Prefisso dell'id delle sessioni di prova (self-test del canale).
const PROBE_PREFIX: &str = "probe-";
/// Versione del protocollo RPC accettata.
const RPC_VERSION: u32 = 1;
/// Unico messaggio per ogni credenziale non valida: non deve dire se e' sbagliato
/// l'id o il segreto (niente oracolo per chi prova a indovinare).
const INVALID_CREDENTIALS: &str = "invalid session credentials";

/// `len` byte dal generatore casuale del sistema operativo, in esadecimale.
fn random_hex(len: usize) -> Result<String, String> {
    let mut bytes = vec![0u8; len];
    getrandom::fill(&mut bytes).map_err(|e| format!("random source unavailable: {e}"))?;
    Ok(hex(&bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Segreto di sessione: `SECRET_BYTES` casuali, 64 caratteri esadecimali.
fn generate_secret() -> Result<String, String> {
    random_hex(SECRET_BYTES)
}

/// Confronto a tempo costante rispetto al contenuto (la lunghezza non e'
/// segreta): niente uscita anticipata al primo byte diverso.
fn secrets_equal(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

/// Segreti delle sessioni aperte: id del tab -> segreto.
#[derive(Default)]
pub struct SessionKeys {
    secrets: Mutex<HashMap<String, String>>,
}

impl SessionKeys {
    /// Emette il segreto della sessione `session`. Se ne esisteva uno (tab
    /// riaperto con lo stesso id) lo sostituisce: quello vecchio non vale piu'.
    pub fn issue(self: &Arc<Self>, session: &str) -> Result<SessionCredential, String> {
        if !is_valid_tab_id(session) {
            return Err("invalid session id".to_string());
        }
        let secret = generate_secret()?;
        lock(&self.secrets).insert(session.to_string(), secret.clone());
        Ok(SessionCredential { keys: Arc::clone(self), session: session.to_string(), secret })
    }

    /// Credenziale usa e getta con un id casuale, per il self-test del canale.
    pub fn issue_probe(self: &Arc<Self>) -> Result<SessionCredential, String> {
        let session = format!("{PROBE_PREFIX}{}", random_hex(PROBE_SUFFIX_BYTES)?);
        self.issue(&session)
    }

    /// Il segreto e' quello attuale della sessione? Falso se vuoto o se la
    /// sessione non ha un segreto.
    pub fn verify(&self, session: &str, secret: &str) -> bool {
        if secret.is_empty() {
            return false;
        }
        lock(&self.secrets).get(session).is_some_and(|expected| secrets_equal(expected, secret))
    }

    /// Toglie il segreto solo se e' ancora esattamente `secret`: la chiusura
    /// in ritardo di una vecchia credenziale non revoca quella del tab
    /// riaperto con lo stesso id.
    fn revoke(&self, session: &str, secret: &str) {
        let mut secrets = lock(&self.secrets);
        if secrets.get(session).is_some_and(|current| secrets_equal(current, secret)) {
            secrets.remove(session);
        }
    }
}

/// Credenziale di una sessione: il segreto vale finche' vive. Al drop viene
/// revocato.
pub struct SessionCredential {
    keys: Arc<SessionKeys>,
    session: String,
    secret: String,
}

impl SessionCredential {
    pub fn session(&self) -> &str {
        &self.session
    }

    pub fn secret(&self) -> &str {
        &self.secret
    }
}

impl Drop for SessionCredential {
    fn drop(&mut self) {
        self.keys.revoke(&self.session, &self.secret);
    }
}

/// Il segreto non compare mai nella rappresentazione di debug.
impl std::fmt::Debug for SessionCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionCredential").field("session", &self.session).field("secret", &"<redacted>").finish()
    }
}

impl crate::ipc::Credential for SessionCredential {
    fn session(&self) -> &str {
        SessionCredential::session(self)
    }

    fn secret(&self) -> &str {
        SessionCredential::secret(self)
    }
}

/// Controlli comuni a ogni richiesta RPC: versione e credenziali della sessione
/// chiamante. Id non valido, sessione sconosciuta e segreto sbagliato danno
/// lo stesso errore.
pub(super) fn authenticate(req: &RpcRequest, keys: &SessionKeys) -> Result<(), String> {
    if req.rpc != RPC_VERSION {
        return Err("unsupported RPC version".to_string());
    }
    if is_valid_tab_id(&req.session) && keys.verify(&req.session, &req.token) {
        Ok(())
    } else {
        Err(INVALID_CREDENTIALS.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> Arc<SessionKeys> {
        Arc::new(SessionKeys::default())
    }

    fn request(rpc: u32, session: &str, token: &str) -> RpcRequest {
        RpcRequest {
            rpc,
            token: token.to_string(),
            session: session.to_string(),
            method: "whoami".to_string(),
            params: Value::Null,
        }
    }

    #[test]
    fn secrets_are_64_hex_chars_and_unique() {
        let (a, b) = (generate_secret().unwrap(), generate_secret().unwrap());
        assert_eq!(a.len(), SECRET_BYTES * 2);
        assert!(a.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
        assert_ne!(a, b);
    }

    #[test]
    fn hex_encodes_every_byte_on_two_digits() {
        assert_eq!(hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
        assert_eq!(hex(&[]), "");
    }

    #[test]
    fn secrets_equal_compares_length_and_content() {
        assert!(secrets_equal("abc", "abc") && secrets_equal("", ""));
        assert!(!secrets_equal("abc", "abd") && !secrets_equal("abc", "abcd") && !secrets_equal("abc", "ABC"));
        assert!(!secrets_equal("abc", "") && !secrets_equal("", "a"));
    }

    #[test]
    fn an_issued_secret_verifies_only_for_its_session() {
        let k = keys();
        let a = k.issue("tab-a").unwrap();
        let b = k.issue("tab-b").unwrap();
        assert_eq!(a.session(), "tab-a");
        assert!(k.verify("tab-a", a.secret()) && k.verify("tab-b", b.secret()));
        assert!(!k.verify("tab-a", b.secret()) && !k.verify("tab-b", a.secret()));
        assert!(!k.verify("tab-a", "") && !k.verify("tab-a", "wrong"));
        assert!(!k.verify("unknown", a.secret()) && !k.verify("", a.secret()));
    }

    #[test]
    fn issuing_again_invalidates_the_previous_secret() {
        let k = keys();
        let old = k.issue("tab-a").unwrap();
        let new = k.issue("tab-a").unwrap();
        assert_ne!(old.secret(), new.secret());
        assert!(!k.verify("tab-a", old.secret()));
        assert!(k.verify("tab-a", new.secret()));
    }

    #[test]
    fn dropping_the_credential_revokes_the_secret() {
        let k = keys();
        let cred = k.issue("tab-a").unwrap();
        let secret = cred.secret().to_string();
        assert!(k.verify("tab-a", &secret));
        drop(cred);
        assert!(!k.verify("tab-a", &secret));
        assert!(lock(&k.secrets).is_empty());
    }

    #[test]
    fn a_late_drop_of_an_old_credential_keeps_the_new_one() {
        let k = keys();
        let old = k.issue("tab-a").unwrap();
        let new = k.issue("tab-a").unwrap();
        drop(old);
        assert!(k.verify("tab-a", new.secret()));
    }

    #[test]
    fn an_invalid_session_id_is_not_issued() {
        let k = keys();
        for id in ["", "a b", "tab/1", "tab\n", &"x".repeat(65)] {
            assert!(k.issue(id).is_err(), "{id:?}");
        }
        assert!(lock(&k.secrets).is_empty());
    }

    #[test]
    fn probe_ids_are_valid_tab_ids_and_unique() {
        let k = keys();
        let (a, b) = (k.issue_probe().unwrap(), k.issue_probe().unwrap());
        assert!(is_valid_tab_id(a.session()) && a.session().starts_with(PROBE_PREFIX));
        assert_ne!(a.session(), b.session());
        assert!(k.verify(a.session(), a.secret()));
        let (session, secret) = (a.session().to_string(), a.secret().to_string());
        drop(a);
        assert!(!k.verify(&session, &secret));
    }

    #[test]
    fn debug_output_hides_the_secret() {
        let k = keys();
        let cred = k.issue("tab-a").unwrap();
        let shown = format!("{cred:?}");
        assert!(shown.contains("tab-a") && !shown.contains(cred.secret()), "{shown}");
    }

    #[test]
    fn authenticate_checks_version_and_credentials() {
        let k = keys();
        let cred = k.issue("tab-1").unwrap();
        assert!(authenticate(&request(1, "tab-1", cred.secret()), &k).is_ok());
        assert_eq!(authenticate(&request(2, "tab-1", cred.secret()), &k), Err("unsupported RPC version".to_string()));
    }

    #[test]
    fn every_bad_credential_gets_the_same_error() {
        let k = keys();
        let cred = k.issue("tab-1").unwrap();
        let _other = k.issue("tab-2").unwrap();
        let bad = [
            request(1, "tab-1", ""),
            request(1, "tab-1", "wrong"),
            request(1, "tab-9", cred.secret()),
            request(1, "a b", cred.secret()),
            request(1, "", cred.secret()),
        ];
        for (i, req) in bad.iter().enumerate() {
            assert_eq!(authenticate(req, &k), Err(INVALID_CREDENTIALS.to_string()), "case {i}");
        }
    }
}
