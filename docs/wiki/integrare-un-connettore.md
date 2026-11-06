# Integrare un nuovo connettore

> **cli-companion non sa come funziona un servizio esterno. Offre dei contratti e un runtime;
> il connettore è un traduttore servizio ↔ DTO e dichiara cosa può fare.**

Questa guida spiega come aggiungere un connettore dell'Agents HUB: una **fonte** (`source`: porta elementi esterni,
come le email di una casella, nei thread secondo le regole dell'utente) o un **canale di contatto** (`contact`:
porta fuori le notifiche dei thread e riporta dentro i messaggi dell'utente). Il progetto è in
[../CONNECTORS.MD](../CONNECTORS.MD); la guida omologa per i coding agent è
[integrare-un-coding-agent.md](integrare-un-coding-agent.md). Vale una regola sola: **il core non nomina mai un
connettore specifico.** Tutto ciò che è proprio di un servizio (id, host, API, formato dei messaggi) vive in
`src-tauri/src/connectors/<id>/`; due test architetturali (`connectors/architecture.rs` e `tests/architecture.test.mjs`)
falliscono se fuori da lì compare un riferimento al connettore.

Indice: [Il modello](#il-modello) · [Il descrittore](#il-descrittore) · [I trait](#i-trait) ·
[Il contesto](#il-contesto) · [Cosa fa il runtime](#cosa-fa-il-runtime-al-posto-tuo) · [Procedura](#procedura-passo-passo) ·
[Test](#test) · [Checklist](#checklist-finale)

## Il modello

Un connettore è una struct (di solito senza campi) che implementa `Connector` e, secondo il tipo, `SourceConnector` o
`ContactConnector` (`connectors/contracts/connector.rs`). Il registro lo tiene in un `ConnectorImpl`:

```rust
pub enum ConnectorImpl {
    Source(&'static dyn SourceConnector),
    Contact(&'static dyn ContactConnector),
}
```

Il connettore **non** consegna nell'HUB, **non** costruisce un client di rete, **non** decide quando girare, quanto
aspettare o come ritentare, **non** salva cursori né id visti: lo fa il supervisore dell'istanza
(`connectors/runtime/`). Il connettore legge dal servizio e restituisce DTO; il resto è del runtime.

Un'**istanza** è un account configurato dall'utente con un connettore (due caselle Gmail = due istanze). Le istanze, le
loro regole (fonti) e i loro collegamenti (canali di contatto) sono in `connectors.json`; i segreti nel portachiavi.
Il registro dice solo cosa è compilato nell'app: una istanza di un connettore che non c'è più resta su disco e si
mostra come `unavailable`.

## Il descrittore

Ogni connettore ha un `ConnectorDescriptor` **statico** (`connectors/descriptor/mod.rs`): è tutto ciò che
l'interfaccia generica sa del connettore e il riassunto revisionabile di cosa può fare.

```rust
pub static DESCRIPTOR: ConnectorDescriptor = ConnectorDescriptor {
    id: "esempio-posta",                // [a-z0-9-], stabile: finisce nei file e negli autori dei messaggi
    kind: ConnectorKind::Source,
    label: "Posta di esempio",          // italiano, per l'interfaccia
    description: "Porta nei thread le email che soddisfano una regola.",
    version: 1,                         // cresce quando cambia la forma di config o filtro
    config_fields: &[
        ConfigField { key: "api_token", label: "Token", help: "", required: true, kind: FieldKind::Secret },
        ConfigField {
            key: "interval_secs", label: "Intervallo (secondi)", help: "", required: false,
            kind: FieldKind::Number { min: 60, max: 3600 },
        },
    ],
    filter_fields: &[ConfigField {
        key: "query", label: "Ricerca", help: "", required: true,
        kind: FieldKind::Text { max_len: 200, placeholder: "da:mario@example.com" },
    }],
    actions: &[Action { id: ActionId::Test, label: "Prova", help: "" }],
    permissions: Permissions {
        hosts: &["api.esempio.com"],                      // gli unici host raggiungibili, solo https
        hub: &[HubCapability::DeliverMessages, HubCapability::WakeSecretaries],
    },
    limits: DescriptorLimits::NONE,
    internal_secret_slots: &["refresh_token"],            // slot oltre ai campi `secret`
};
```

- **Campi** (`FieldKind`): `Text`, `Secret`, `Number`, `Bool`, `Select`, `Workspace`, `Thread`. L'interfaccia
  generica e la validazione di tipi e limiti si costruiscono da qui. Un campo `Secret` **non sta mai in `config`**:
  va nel portachiavi (`connector_set_secret`) e i comandi rifiutano un segreto nella configurazione. Un campo
  `interval_secs` (secondi) o `interval_minutes` (minuti) di tipo numero, se c'è, regola il ritmo delle fonti; se non
  è impostato vale `limits.default_interval_secs` del descrittore, in ogni caso minimo 60 secondi. Gmail è
  l'implementazione di riferimento delle fonti.
- **Azioni** (`ActionId`): `Authorize` (consenso OAuth), `Pair` (abbinamento di un account, solo `contact`), `Test`
  (verifica credenziali e rete). Alla funzione `action` del connettore arrivano solo quelle dichiarate.
- **Permessi**: gli `hosts` sono l'allowlist del client HTTP; le capacità verso l'HUB (`HubCapability`) sono
  `DeliverMessages` e `WakeSecretaries` per una fonte, `ReceiveNotices` e `SendAsUser` per un contatto. Il runtime le
  fa rispettare: senza `WakeSecretaries` una regola con `wake` diverso da `none` è rifiutata alla creazione e, se
  arrivasse comunque, ignorata alla consegna.
- **Versione e migrazione**: quando cambia la forma della config o del filtro, `version` sale e
  `Connector::migrate_config(from, config)` fa **un passo** da `from` a `from + 1` (pura). Il runtime la applica
  passo per passo al caricamento, a config e filtri delle regole insieme; se fallisce l'istanza resta com'era e va in `error`.

Il descrittore si valida da solo (`descriptor::validate::validate_descriptor`, nel test di conformità): id e chiavi ben
formati, nessun duplicato, host validi, capacità coerenti con il tipo, `pair` solo per i contatti.

## I trait

```rust
pub trait Connector: Send + Sync {
    fn descriptor(&self) -> &'static ConnectorDescriptor;
    fn migrate_config(&self, from: u32, config: Value) -> Result<Value, String>;
    fn validate_config(&self, config: &Value) -> Result<(), String>;   // solo la parte semantica
    fn action<'a>(&'a self, ctx: &'a ConnectorContext, action: ActionId)
        -> BoxFuture<'a, Result<ActionOutcome, ConnectorError>>;
}

pub trait SourceConnector: Connector {
    fn validate_filter(&self, filter: &Value) -> Result<(), String>;
    fn poll<'a>(&'a self, ctx: &'a ConnectorContext, routes: &'a [ActiveRoute], cursor: Option<Value>)
        -> BoxFuture<'a, Result<PollBatch, ConnectorError>>;
}

pub trait ContactConnector: Connector {
    fn wants_notice(&self, config: &Value, category: NoticeCategory) -> bool;
    fn receive<'a>(&'a self, ctx: &'a ConnectorContext, cursor: Option<Value>)
        -> BoxFuture<'a, Result<ReceiveBatch, ConnectorError>>;
    fn send<'a>(&'a self, ctx: &'a ConnectorContext, out: &'a OutboundMessage)
        -> BoxFuture<'a, Result<Vec<String>, ConnectorError>>;
    fn fetch_file<'a>(&'a self, ctx: &'a ConnectorContext, file: &'a RemoteFile)
        -> BoxFuture<'a, Result<IncomingFile, ConnectorError>>;
    fn pairing_reply<'a>(&'a self, ctx: &'a ConnectorContext, outcome: &'a PairingOutcome, chat: &'a str)
        -> BoxFuture<'a, Result<(), ConnectorError>>;

    // Sotto-chat, facoltative: le implementazioni predefinite dicono «non supportate».
    fn subchats(&self, config: &Value, chat: &str) -> bool { false }
    fn open_subchat<'a>(&'a self, ctx: &'a ConnectorContext, chat: &'a str, title: &'a str)
        -> BoxFuture<'a, Result<String, ConnectorError>>;
    fn rename_subchat<'a>(&'a self, ctx: &'a ConnectorContext, chat: &'a str, subchat: &'a str, title: &'a str)
        -> BoxFuture<'a, Result<(), ConnectorError>>;
    fn close_subchat<'a>(&'a self, ctx: &'a ConnectorContext, chat: &'a str, subchat: &'a str)
        -> BoxFuture<'a, Result<(), ConnectorError>>;
    fn delete_subchat<'a>(&'a self, ctx: &'a ConnectorContext, chat: &'a str, subchat: &'a str)
        -> BoxFuture<'a, Result<(), ConnectorError>>;
}
```

Non ci sono implementazioni di default che nascondano una lacuna: se aggiungi un metodo ai trait del core, ogni
connettore smette di compilare finché non decide. L'unica eccezione sono le **sotto-chat** dei canali di contatto,
una capacità facoltativa: chi non le ha non scrive nulla (`subchats` vale `false` e il runtime non chiede le altre;
se ci arrivasse, rispondono `Fatal`).

- **`poll`** (fonte): un giro. Legge dal servizio gli elementi nuovi per le regole attive a partire da `cursor`
  (`None` = si parte da «adesso»: gli arretrati non entrano) e restituisce `PollBatch { items, cursor, more }`.
  Ogni `SourceItem` dice per quale regola (`route`) è stato letto, con `external_id` (1..256 caratteri, base della
  dedupe), `conversation_key` (per le regole `per_conversation`), `title`, `sender`, `body` già pronto e gli allegati
  come byte. `more: true` fa partire subito il giro successivo. **Non consegna e non salva il cursore.**
- **`receive`** (contatto): long polling con un timeout del servizio più breve di `limits::RECEIVE_TIMEOUT`.
  Restituisce i messaggi `InboundMessage` già tradotti — `pairing_code` se il testo è un invito ad abbinare,
  `command` (`ContactCommand`) se è un comando, `reply_to_external` per una risposta, `sender_name`, gli allegati come
  `RemoteFile` **non scaricati** — e il cursore nuovo. I messaggi che il canale non gestisce (gruppi senza
  sotto-chat, modificati, sticker) non diventano `InboundMessage`. Comandi, collegamenti, thread corrente e testi
  delle risposte sono del runtime (`runtime/contact/`), uguali per ogni canale.
- **`send`**: manda un testo a una chat collegata, spezzandolo al limite del servizio e rispettandone la frequenza, e
  ritorna l'id esterno di ogni parte (servono per le risposte).
- **`fetch_file`**: scarica un allegato; il runtime lo chiede solo per un account collegato ed entro i limiti dell'HUB.
- **`wants_notice`**: pura, dice se la configurazione dell'istanza vuole le notifiche di un tipo (`NoticeCategory`).
- **`mirrors_app_messages`** (facoltativa, predefinita: no): pura, dice se la configurazione vuole l'eco dei messaggi
  scritti nell'app nella sotto-chat del thread (`docs/CONNECTORS.MD` §10); il runtime la chiede solo per le chat con
  le sotto-chat e invia solo in una sotto-chat che esiste già.
- **`pairing_reply`**: risponde a chi ha presentato un codice (benvenuto, codice errato, troppi tentativi).
- **Sotto-chat** (facoltative; `docs/CONNECTORS.MD` §10, «Sotto-chat»): se il servizio ha conversazioni separate
  dentro una chat (i topic di un forum), `subchats(config, chat)` dice per quali chat valgono (pura, può dipendere
  dalla chat) e `open_subchat` ne apre una col titolo che sceglie il runtime («canale › thread»), tagliandolo al limite
  del servizio, e ne ritorna l'id opaco (lettere, cifre e `-_.:`, al più 64 caratteri, o il runtime lo scarta). In
  `receive` metti in `InboundMessage.subchat` la sotto-chat del messaggio (`None` per quella principale); in `send`
  scrivi in `OutboundMessage.subchat`. Se il servizio dice che la sotto-chat non esiste più (eliminata a mano),
  `send` risponde `ConnectorError::SubchatGone`: il runtime la toglie dalla sua mappa e la ricrea una volta. La mappa
  thread ↔ sotto-chat è del runtime, non del connettore. Per tenerle allineate ai thread implementa anche
  `close_subchat` (thread concluso), `reopen_subchat` (messaggio o notifica successivi) e `delete_subchat` (thread o
  workspace eliminato): traducono e basta, decide il runtime; una sotto-chat già sparita risponde `SubchatGone`, che
  per il runtime è un successo.
- **`action`**: `Authorize` (di solito apre il browser e salva il refresh token con `ctx.set_secret`), `Pair` (il
  codice emesso dal runtime è in `ctx.pairing_code()`: va mostrato all'utente, il connettore può metterlo in un
  invito e restituirlo in `ActionOutcome.detail`), `Test`.

**Errori**: ogni funzione che parla col servizio restituisce un `ConnectorError`, e la classe decide cosa fa il runtime:

| Classe | Quando | Effetto |
|---|---|---|
| `Transient(msg)` | rete, timeout, 5xx | stato `backoff`, riprova con attesa esponenziale (5 s → 10 min) |
| `RateLimited { retry_after }` | 429 | rispetta `Retry-After` (mai meno del backoff, al massimo 1 ora) |
| `Auth(msg)` | 401, 403, `invalid_grant` | stato `needs_auth`: si ferma finché l'utente non cambia segreto o rifà `authorize` |
| `Config(msg)` | configurazione inutilizzabile | stato `error`, fermo fino a una modifica dell'istanza |
| `Fatal(msg)` | errore non recuperabile | stato `error` |
| `SubchatGone` | `send` in una sotto-chat che il servizio non ha più | il runtime ricrea la sotto-chat una volta, poi ripiega sulla chat principale; il supervisore non si ferma |

I messaggi sono già ripuliti: **mai** segreti, URL con token, corpi di risposta o contenuti degli elementi. All'utente
arriva solo un testo generico scelto dalla classe; il dettaglio va nel log (solo id dell'istanza e classe).
`backoff::parse_retry_after` legge `Retry-After` (secondi o data HTTP).

## Il contesto

`ConnectorContext` è l'unica cosa che il connettore riceve dal runtime. Offre solo:

| Metodo | Cosa dà |
|---|---|
| `config()` | la configurazione già migrata e validata; mai segreti |
| `secret(slot)`, `set_secret(slot, valore)` | i segreti dei soli slot dichiarati (campi `secret` e `internal_secret_slots`) |
| `http()` | il **client HTTP del contesto**: rifiuta ogni host non dichiarato in `permissions.hosts`, solo `https`, redirect solo verso host dichiarati, timeout e tetto alla dimensione della risposta, errori ripuliti (mai l'URL) |
| `now()`, `random_bytes(n)` | ora e casualità iniettate: la logica si prova senza rete né orologio |
| `state_get(chiave)`, `state_set(chiave, valore)` | valori privati del connettore nello stato dell'istanza (poche chiavi, dimensione limitata) |
| `pairing_code()` | il codice monouso emesso per l'azione `pair`, altrimenti `None` |
| `deliver(..)`, `send_as_user(..)`, `send_as_user_files(..)` | l'accesso all'HUB, solo con la capacità dichiarata: senza, «permesso non dichiarato». Per le azioni; `poll` e `receive` restituiscono e il supervisore consegna |

Quindi: **niente `reqwest`** nel codice di un connettore (lo vieta il test architetturale), niente segreti in `config`,
niente stato su file proprio.

## Cosa fa il runtime al posto tuo

- Un **supervisore per istanza** abilitata, solo con l'HUB acceso; si ferma con la disabilitazione, l'eliminazione,
  l'HUB spento, l'uscita dall'app.
- Per le fonti: un giro ogni `interval_secs`/`interval_minutes` (o `default_interval_secs`; minimo 60 s), al più 20 elementi per giro, consegna con
  `deliver_external`, **poi** salva cursore e id visti: almeno una volta, con dedupe per `(regola, external_id)` e per
  `origin` nel thread. Al più un risveglio per thread per giro (dall'ultimo elemento che lo chiede).
- Una regola `existing` il cui thread sparisce si disabilita da sola («thread non più disponibile»); il suo elemento non
  si perde.
- Per i contatti: ricezione continua, abbinamento con codici monouso (12 caratteri, 10 minuti, al più 5 tentativi ogni
  10 minuti), messaggi dei collegamenti al thread corrente con `send_user_message`, notifiche dell'HUB inoltrate con
  una coda limitata.
- Timeout (30 s per chiamata, 2 min per giro), backoff, stato di salute (`connector-status`), salvataggio dello stato su
  disco.
- I comandi Tauri e l'interfaccia generica (descrittori → moduli): non si scrive nulla per il connettore.

## Procedura passo passo

1. **Cartella**: `src-tauri/src/connectors/<nome>/` (modulo `mod.rs`, file per responsabilità; il servizio, i DTO
   dell'API e il parsing stanno qui, le funzioni pure separate dall'I/O e provate con risposte registrate, senza rete).
2. **Descrittore** `static` come sopra: campi, azioni, `hosts`, capacità, `version`.
3. **Implementa** `Connector` + `SourceConnector` o `ContactConnector`. Tieni `validate_config` e `validate_filter` puri
   (la validazione di tipi e limiti la fa già il core: qui solo il significato). Per `migrate_config` scrivi un passo
   per ogni versione precedente alla corrente (con `version: 1` non ce ne sono: rispondi `Err`).
4. **Dichiara il modulo** in `connectors.rs` (`mod <nome>;`) e **registralo** con una riga in `CONNECTORS` di
   `connectors/registry.rs`: `ConnectorImpl::Source(&<nome>::CONNECTOR)`. È l'unico punto del core che lo nomina.
5. **Test architetturali**: una riga in `KNOWN` di `connectors/architecture.rs` (cartella e id del descrittore) e una in
   `CONNECTORS` di `tests/architecture.test.mjs`. Il frontend è generico: non va toccato.
6. **Togli gli `expect(dead_code)`** che esistevano perché il registro era vuoto: con il primo connettore reale
   clippy (`-D warnings`) segnala come «unfulfilled expectation» quelli in `contracts/connector.rs`, `contracts/error.rs`,
   `contracts/context.rs`, `descriptor/mod.rs`, `limits.rs`, `state.rs` e `runtime/http.rs`, `runtime/backoff.rs` che
   ora sono raggiungibili. Quelli che il tuo connettore non usa ancora (per esempio `with_max_body`, o `SubchatGone` e
   `rename_subchat` senza sotto-chat) restano.
7. **Documenta**: la riga nella tabella delle fasi di `docs/CONNECTORS.MD` e, se serve, la sezione del connettore.

## Test

- **Conformità** (`connectors/conformance.rs`): ogni connettore registrato è verificato da solo. Controlla che il
  descrittore sia valido e coerente con il tipo (una fonte dichiara `deliver_messages`; `interval_secs` ha minimo ≥ 60
  s, `default_interval_secs` rispetta i minimi), che il tipo del trait coincida con `descriptor().kind`, che la migrazione riesca dalla versione 1 alla corrente,
  che la configurazione e il filtro di esempio passino la validazione generica e semantica e che nessuna funzione
  pura vada in panico con input ostili (nulli, enormi, unicode, annidati). Non devi scrivere altro per questo.
- **Architettura** (`connectors/architecture.rs`, `tests/architecture.test.mjs`): nessun file fuori dalla cartella del
  connettore ne cita il modulo o l'id (nemmeno in una stringa); il connettore importa da `crate::hub::` solo
  `ingress`, `attachments`, `Author`, `Message`, `MessageOrigin`, `Channel`, `HubNotice` e non usa `reqwest`.
- **Del connettore**: parsing delle risposte del servizio, costruzione delle richieste, mappatura errori → classi,
  migrazione, validazioni. Per provare il comportamento end-to-end con il runtime usa i finti di
  `connectors/testing.rs` (`FakeSource`, `FakeContact` in `testing/contact.rs`, che simula anche le sotto-chat con
  `with_subchats_in`, `open_error` e `delete_remotely`, `FakeHub`, `FakeClock`) e il registro di test per thread
  (`registry::test_registry::Registered`). Nessun test parla con la rete.

## Checklist finale

- [ ] Descrittore completo; solo gli host e le capacità che servono davvero.
- [ ] Nessun segreto in `config`, nei log, negli errori o nei DTO.
- [ ] Nessun `reqwest`, nessun `unwrap`/`expect`/indicizzazione senza controllo su dati esterni (in release un panico
      chiude l'app: `panic = "abort"`).
- [ ] `poll`/`receive` non consegnano e non salvano nulla; i cursori sono quelli restituiti.
- [ ] Errori classificati (`Auth` per 401/403, `RateLimited` per 429, `Transient` per la rete).
- [ ] Riga in `CONNECTORS` (`registry.rs`), in `KNOWN` (`architecture.rs`) e in `tests/architecture.test.mjs`.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (da Git Bash con `env -u WSLENV`),
      `npm run bindings` (se sono cambiati DTO), `npm run lint`, `npm run typecheck`, `npm test`.
- [ ] `docs/CONNECTORS.MD` e `docs/DESIGN.MD` aggiornati.
