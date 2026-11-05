//! I tetti del runtime dei connettori, in un posto solo. Un connettore puo'
//! dichiarare limiti piu' stretti nel suo descrittore, mai piu' larghi.

use std::time::Duration;

/// Istanze configurabili in tutto.
pub const MAX_INSTANCES: usize = 20;
/// Regole per istanza.
pub const MAX_ROUTES_PER_INSTANCE: usize = 50;
/// Collegamenti per istanza (canali di contatto).
pub const MAX_LINKS_PER_INSTANCE: usize = 5;

/// Id visti per istanza (dedupe): i piu' vecchi escono per primi.
pub const SEEN_MAX: usize = 5000;
/// Voci di ciascuna mappa dello stato (chiave di conversazione -> thread, id
/// esterno <-> id del messaggio dell'HUB): le meno recenti escono per prime.
pub const MAP_MAX: usize = 2000;
/// Chiavi private di un connettore nel suo stato.
pub const CUSTOM_KEYS_MAX: usize = 64;
/// Dimensione massima, serializzata, di un valore privato di un connettore.
pub const CUSTOM_VALUE_MAX: usize = 64 * 1024;

/// Intervallo minimo fra due giri di una fonte.
pub const MIN_INTERVAL: Duration = Duration::from_secs(60);
/// Elementi consegnati per giro.
pub const ITEMS_PER_ROUND: usize = 20;
/// Tetto di una singola chiamata al servizio.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// Tetto di una ricezione a lungo polling di un canale di contatto: l'attesa del
/// servizio (al piu' `LONG_POLL_MAX`) piu' un margine.
pub const RECEIVE_TIMEOUT: Duration = Duration::from_secs(70);
/// L'attesa massima che un connettore puo' chiedere al servizio in un long polling.
pub const LONG_POLL_MAX: Duration = Duration::from_secs(55);
/// Tetto di un giro intero.
pub const ROUND_TIMEOUT: Duration = Duration::from_secs(120);
/// Tetto di un'azione dell'interfaccia (prova, abbinamento, consenso OAuth: l'utente puo' impiegare del tempo).
pub const ACTION_TIMEOUT: Duration = Duration::from_secs(300);
/// Backoff: primo tentativo e massimo.
pub const BACKOFF_MIN: Duration = Duration::from_secs(5);
pub const BACKOFF_MAX: Duration = Duration::from_secs(600);
/// Attesa massima che un servizio puo' imporre con `Retry-After`.
pub const RETRY_AFTER_MAX: Duration = Duration::from_secs(3600);

/// Dimensione massima di una risposta HTTP normale.
pub const RESPONSE_MAX: usize = 1024 * 1024;
/// Dimensione massima di una risposta con un allegato: il limite dell'HUB piu' un margine.
pub const RESPONSE_MAX_FILES: usize = 10 * 1024 * 1024 + 256 * 1024;
/// Redirect seguiti al massimo per richiesta.
pub const REDIRECTS_MAX: usize = 5;

/// Lunghezza massima, in caratteri, di una etichetta di istanza o di regola.
pub const LABEL_MAX: usize = 60;
/// Lunghezza massima della configurazione o del filtro serializzati.
pub const CONFIG_JSON_MAX: usize = 16 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limits_are_coherent() {
        assert_eq!((MAX_INSTANCES, MAX_ROUTES_PER_INSTANCE, MAX_LINKS_PER_INSTANCE), (20, 50, 5));
        assert_eq!((SEEN_MAX, MAP_MAX), (5000, 2000));
        assert!(BACKOFF_MIN < BACKOFF_MAX && BACKOFF_MAX < RETRY_AFTER_MAX);
        assert!(CALL_TIMEOUT < ROUND_TIMEOUT);
        const { assert!(RESPONSE_MAX < RESPONSE_MAX_FILES) };
        const { assert!(RESPONSE_MAX_FILES > crate::hub::attachments::ATTACHMENT_SIZE_MAX as usize) };
        assert_eq!(MIN_INTERVAL.as_secs(), 60);
    }
}
