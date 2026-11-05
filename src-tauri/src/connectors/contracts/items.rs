//! I dati che un connettore scambia con il runtime: cio' che legge dal servizio
//! (elementi di una fonte, messaggi in entrata di un canale di contatto) e cio'
//! che il runtime gli chiede di mandare. Sono tipi Rust; il connettore e' un
//! traduttore servizio <-> questi DTO, la consegna nell'HUB e' del runtime.

use crate::hub::attachments::IncomingFile;
use serde_json::Value;

/// Una regola attiva di una fonte, come la vede `poll`.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveRoute {
    /// L'id della regola (`rte-...`): `SourceItem::route` lo riporta.
    pub id: String,
    /// Il filtro, gia' migrato e validato.
    pub filter: Value,
}

/// Un elemento letto da una fonte per una regola.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceItem {
    /// L'id della regola per cui e' stato letto.
    pub route: String,
    /// L'id dell'elemento nel servizio (1..=256 caratteri): base della dedupe.
    pub external_id: String,
    /// La chiave di conversazione del servizio (es. il `threadId` di Gmail), per `per_conversation`.
    pub conversation_key: Option<String>,
    /// Il titolo (l'oggetto): intitola il thread di `new_per_item`.
    pub title: String,
    /// Il mittente come si presenta (ripulito dall'HUB).
    pub sender: String,
    /// Il testo gia' pronto per il thread (intestazione, riga vuota, corpo).
    pub body: String,
    pub files: Vec<IncomingFile>,
}

/// L'esito di un giro di `poll`.
#[derive(Debug, Clone, PartialEq)]
pub struct PollBatch {
    pub items: Vec<SourceItem>,
    /// Il cursore da salvare dopo la consegna, e da riportare al giro successivo.
    pub cursor: Value,
    /// Altri elementi sono pronti: il prossimo giro parte subito.
    pub more: bool,
}

/// Un messaggio arrivato a un canale di contatto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundMessage {
    /// L'id del messaggio nel servizio: base della dedupe e di `origin.external_id`.
    pub external_id: String,
    /// L'account esterno che l'ha scritto (stringa opaca).
    pub account: String,
    /// La chat in cui rispondere.
    pub chat: String,
    /// Come si presenta il mittente (nome o username), per il collegamento; vuoto se ignoto.
    pub sender_name: String,
    pub text: String,
    /// L'id esterno del messaggio a cui risponde, se e' una risposta.
    pub reply_to_external: Option<String>,
    /// Gli allegati, ancora da scaricare con `ContactConnector::fetch_file`.
    pub files: Vec<RemoteFile>,
    /// Il codice di abbinamento, se il messaggio e' un invito ad abbinare (`/start <codice>`).
    pub pairing_code: Option<String>,
    /// Il comando, se il messaggio lo e': allora `text` non va nel thread.
    pub command: Option<ContactCommand>,
}

/// Un allegato di un messaggio in entrata, non ancora scaricato: il runtime lo
/// chiede al connettore (`ContactConnector::fetch_file`) solo per un account
/// collegato ed entro i limiti degli allegati dell'HUB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    /// L'id del file nel servizio (opaco, per `fetch_file`).
    pub id: String,
    /// Il nome come lo mostra il servizio (lo ripulisce l'HUB).
    pub name: String,
    /// La dimensione in byte dichiarata dal servizio, se la conosce.
    pub size: Option<u64>,
}

/// Un comando scritto dall'utente nella chat (`/threads`, `/use 2`...). Il
/// connettore lo riconosce secondo la sintassi del servizio; il runtime lo
/// esegue e risponde (docs/CONNECTORS.MD §10). L'argomento e' il testo dopo il
/// comando, senza spazi ai bordi (vuoto se manca).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContactCommand {
    Help,
    Threads,
    Use(String),
    Workspaces(String),
    New(String),
    Status,
    Mute(String),
    Unmute,
    Unlink,
    /// Un comando che il connettore non conosce: il runtime risponde con l'aiuto.
    Unknown,
}

/// L'esito di `receive`: i messaggi e il cursore da salvare dopo la consegna.
#[derive(Debug, Clone, PartialEq)]
pub struct ReceiveBatch {
    pub messages: Vec<InboundMessage>,
    pub cursor: Value,
}

/// Un messaggio da mandare a una chat collegata. Il testo puo' superare il limite
/// del servizio: il connettore lo spezza (`send` ritorna l'id di ogni parte).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundMessage {
    pub chat: String,
    /// Testo semplice, senza markup.
    pub text: String,
    /// L'id esterno del messaggio a cui legare la risposta.
    pub reply_to_external: Option<String>,
}

/// Cosa e' successo a un tentativo di abbinamento, perche' il connettore risponda come crede.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingOutcome {
    /// Abbinato: benvenuto.
    Linked { display_name: String },
    /// Codice sbagliato o scaduto.
    Rejected,
    /// Troppi tentativi.
    TooManyAttempts,
}

/// L'esito di un'azione dell'interfaccia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionOutcome {
    /// Il messaggio per l'utente, in italiano.
    pub message: String,
    /// Un dato da mostrare (un link di abbinamento, il nome del bot), se c'e'.
    pub detail: Option<String>,
}
