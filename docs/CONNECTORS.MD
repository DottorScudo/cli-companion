# Connettori dell'Agents HUB

Documento di progetto e riferimento del codice di `src-tauri/src/connectors/`: i **connettori** estendono
l'[Agents HUB](HUB.MD) verso l'esterno. Sono consegnate le fasi 1 (fondamenta nell'HUB), 2 (runtime, registro,
comandi Tauri), 3 (interfaccia generica), 4 (Telegram, il primo connettore del registro) e 5 (Gmail, la fonte di riferimento).
Sono consegnate le fasi 1…5 (vedi "Fasi"). Dove una decisione cambia un comportamento descritto in HUB.MD lo si dice esplicitamente
come **cambiamento previsto**; finché la fase che lo introduce non è consegnata vale HUB.MD. La ricetta per
aggiungere un connettore è in [wiki/integrare-un-connettore.md](wiki/integrare-un-connettore.md).

Due tipi, distinti nel modello, nel codice e nell'interfaccia:

| Tipo (codice) | Nell'interfaccia | Verso | Esempio | Cosa fa |
|---|---|---|---|---|
| `source` | **Fonte** | solo in ingresso | Gmail | fa entrare dati esterni in un thread di un workspace secondo una regola (es. nel workspace X tutte le email con mittente `mario@gmail.com`); ogni elemento è un possibile **risveglio** del segretario |
| `contact` | **Canale di contatto** | bidirezionale | Telegram, (poi) Discord | porta all'utente, fuori dall'app, le notifiche dei thread e gli permette di scrivere ai segretari nei vari thread, come se fosse nell'HUB |

L'obiettivo è un ecosistema in cui aggiungere un connettore nuovo sia una ricetta ripetibile, come aggiungere un
coding agent ([wiki/integrare-un-coding-agent.md](wiki/integrare-un-coding-agent.md)): un modulo compilato dietro un
contratto, un registro, test di conformità e test architetturali.

**Nomi.** Nel codice "plugin" indica già il plugin di Claude Code `cli-companion-notify` (`PluginUpdateDialog.tsx`,
`mcp/`) e "channel" il canale dell'HUB (`hub/channel.rs`). Per questo nel codice si dice **connector** (modulo
`src-tauri/src/connectors/`, tipi `source` e `contact`) e mai *plugin* né *channel* per queste cose. Nell'interfaccia:
«Connettori», «Fonti», «Canali di contatto».

## 1. Stato attuale

### Come si sveglia un segretario oggi

Due percorsi, che finiscono entrambi in `Sessions` (`hub/engine/sessions.rs`, puro) e nelle code dei thread
(`hub/engine/runtime.rs`):

**(a) Messaggio dell'utente.** Il comando `hub_send` (`hub/commands.rs`) valida canale e HUB acceso, poi in
`crate::blocking`:

1. salva il messaggio con `append_user_message` (`hub/append.rs`) e autore `Author::User`; gli allegati, già nello
   staging, passano alla cartella del thread;
2. `deliver_if_answer` (`hub/direct_answer.rs`): se il messaggio ha `reply_to` su un evento `task_question` ancora
   in attesa, il testo va al worker così com'è (azione `answer_question` registrata da `lib.rs`) e nessun segretario
   si sveglia;
3. altrimenti `SecretaryEngine::on_message` (`hub/engine/runtime.rs`) → `Sessions::route`: le menzioni `@nome`
   (`engine/mentions.rs`) di segretari configurati e ammessi nel canale, oppure, senza menzioni, il primo candidato
   svegliabile tra autore del messaggio a cui si risponde, titolare della sessione più recente, titolare del thread e
   ultimo segretario che vi ha scritto (`sessions::Fallback`); se nessuno, un avviso di sistema nel thread.
   `route` esce subito se `message.author != Author::User`. Il turno si accoda con
   `WakeCause::Message`.

Prima della fase 1 la logica di questi tre passi viveva **dentro il comando Tauri**; oggi `hub_send` è un adattatore
su `hub::send::send_user_message`, riusabile dai canali di contatto.

**(b) Eventi della board.** `automation::AutomationState::publish` → `Host::board_published` (`TauriHost`) →
`hub::board_published` → istantanea compatta (`events/snapshot.rs`) in coda a `HubFeed` (`events/feed.rs`, thread
`hub-board-events`) → `events/diff.rs` (eventi) + `events/route.rs` (thread, testi, notifiche) → `Routed` con gli
`EventWake { thread_id, secretary, trigger }` dei thread con un titolare (`HubThread.owner`) → dopo il salvataggio e
fuori dal lock `SecretaryEngine::on_events` → `Sessions::route_event` (solo il titolare, se ancora configurato e
ammesso) → turno con `WakeCause::BoardEvent`, che non può usare `decide_proposal` né `relay_answer`. Al massimo un
risveglio per thread a ogni passo della board.

**Notifiche.** Chi scrive un messaggio che notifica chiama `HubNotifier::notify` (`hub/notify/dispatch.rs`, stato
Tauri): raggruppa per thread nella finestra `GROUP_WINDOW` (3 s), poi emette l'evento `hub-notify` con un
`HubNotice { channel, thread_id, title, body, sound }`. Lo fa **solo verso il frontend** e solo se
`enabled && notifications` (`notifications_on`); `body` è già ridotto a due righe da 120 caratteri.

### Cosa mancava

Tutto consegnato nelle fasi 1 e 2 (`Author::External`, `hub::ingress`, `hub::send`, `set_notice_sink`,
`set_lifecycle_listener`, il modulo `connectors/`).

| Mancava | Perché serve |
|---|---|
| Un autore non-agente esterno | `Author` è `user \| system \| bot` (`hub/model.rs`): un'email non è dell'utente né di un segretario |
| Un'API Rust per depositare un messaggio esterno in un thread | con allegati da byte (non dallo staging del frontend), dedupe e risveglio secondo una regola, non secondo le menzioni |
| Un'API Rust per scrivere «come utente» da un canale esterno | la logica di `hub_send` (salvataggio, risposta diretta, motore) è nel comando |
| Un'uscita delle notifiche verso destinatari diversi dal frontend | `HubNotifier` emette solo `hub-notify` |
| Un runtime in background | task di lunga durata con cursori persistenti, backoff, stato di salute, arresto pulito |
| Un posto per configurazioni e segreti per istanza | oggi c'è un solo segreto (`hub-provider-api-key`) e nessuna configurazione per account esterni |

### Cosa c'è già e si riusa

- `secrets::SecretStore` / `OsKeyring` (`src-tauri/src/secrets.rs`): portachiavi del SO, servizio `cli-companion`;
  `MemorySecrets` nei test.
- `store/`: `JsonFile` con busta versionata `{schema_version, data}`, migrazioni pure, backup dei file corrotti,
  `write_atomic`, avvisi `store-warning`; `JsonFile::named` per i file con nome noto solo a runtime.
- `reqwest` 0.13 con rustls e provider `ring` (`src-tauri/Cargo.toml`, già usati da async-openai e dall'updater):
  nessun crate HTTP nuovo; si aggiungeranno solo le feature di reqwest che servono (per esempio `json`, `query`,
  `multipart`).
- `tauri::async_runtime` (spawn, `JoinHandle::abort`), come le code dei thread del motore.
- `hub/attachments/`: `storage::stage_bytes` (nome ripulito, MIME dalle firme, limite di dimensione),
  `ATTACHMENTS_MAX` = 5, `ATTACHMENT_SIZE_MAX` = 10 MB, pulizia dei file dei messaggi potati.
- `getrandom` (già usato da `automation/auth.rs`) per i codici di abbinamento e lo stato OAuth; `sha2` per il
  PKCE (`S256`) del consenso OAuth.
- `tauri-plugin-opener` per aprire il browser del consenso OAuth.
- Il profilo release ha `panic = "abort"`: un panico in un connettore **chiude l'app** (vedi "Sicurezza").

## 2. Concetti

**Connettore** (`Connector`). Codice compilato nell'app, in `connectors/<id>/`, con un **descrittore** statico (vedi
§3). Esiste se è nel registro (`connectors/registry.rs`); da solo non fa nulla.

**Istanza** (`ConnectorInstance`). Un account configurato dall'utente con un connettore: un bot Telegram, una casella
Gmail. Ce ne possono essere più d'una per connettore.

| Campo | Regole |
|---|---|
| `id` | `con-<millisecondi>-<n>`, come gli id di thread e messaggi; valido per `text::valid_ref_id` (finisce in un nome di file e nel conto del portachiavi) |
| `connector` | id di un connettore del registro; un id sconosciuto (connettore tolto da una versione successiva) lascia l'istanza su disco com'è, ferma, con stato `unavailable` («connettore non disponibile in questa versione»): non si abilita e non perde né config né segreti |
| `label` | 1..60 caratteri, una riga; scelto dall'utente (es. «Gmail lavoro») |
| `enabled` | `false` alla creazione: si abilita dopo la configurazione e il test |
| `config_version` | la `version` del descrittore con cui la config è stata scritta |
| `config` | oggetto JSON validato dal connettore (`validate_config`) contro i campi del descrittore; **mai segreti** |
| `notices` | solo canali di contatto: interruttore generale e per tipo di notifica (vedi §10) |
| `created_at`, `updated_at` | millisecondi Unix |

I **segreti** dell'istanza (token del bot, refresh token OAuth, client secret) stanno solo nel portachiavi, servizio
`cli-companion`, conto `connector-<istanza>-<slot>` (es. `connector-con-1730000000000-1-bot_token`). `slot` è il nome
del campo `secret` del descrittore o uno slot interno dichiarato dal connettore (`oauth_refresh_token`), `[a-z0-9_]`.
Il frontend vede solo `set: bool` per slot.

**Regola di instradamento** (`ConnectorRoute`, solo fonti). Dice quali elementi di un'istanza entrano in quale thread
e chi svegliano.

| Campo | Regole |
|---|---|
| `id` | `rte-<millisecondi>-<n>` |
| `instance` | un'istanza di tipo `source` |
| `label` | 1..60 caratteri, una riga |
| `filter` | oggetto JSON validato dal connettore (`validate_filter`) contro i campi del filtro del descrittore (Gmail: `{ "query": "from:mario@gmail.com" }`) |
| `channel` | `general` o l'id di un workspace esistente (`Channel`) |
| `thread` | `{ "mode": "existing", "thread_id": "thr-…" }` (un thread scelto dall'utente; gli elementi vanno tutti lì) \| `{ "mode": "new_per_item" }` (un thread nuovo per elemento, intitolato con il titolo dell'elemento) \| `{ "mode": "per_conversation" }` (la chiave di conversazione esterna dell'elemento, es. il `threadId` di Gmail, sceglie il thread; la mappa chiave → thread la tiene il connettore nel suo stato, vedi §6) |
| `wake` | `"none"` (solo messaggio nel thread) \| `"owner"` (il titolare del thread, se c'è) \| `"secretary:<nome>"` (un segretario preciso, che deve essere configurato e ammesso nel canale al momento della consegna) |
| `enabled` | `true` alla creazione |

Un elemento che soddisfa più regole della stessa istanza si consegna **una volta per regola** (thread diversi sono
destinazioni diverse); due regole che puntano allo stesso thread lo ricevono una volta sola (dedupe per thread, §7), e
due regole che aprono un thread nuovo nello stesso canale anche (dedupe nel canale, §7): l'elemento resta nel thread
aperto dalla prima.

**Collegamento** (`ContactLink`, solo canali di contatto). Lega un account esterno all'utente dell'app.

| Campo | Regole |
|---|---|
| `id` | `lnk-<millisecondi>-<n>` |
| `instance` | un'istanza di tipo `contact` |
| `external_account` | id dell'utente esterno (Telegram: `from.id`), stringa opaca, al massimo 128 caratteri |
| `external_chat` | id della chat privata in cui scrivere (Telegram: `chat.id`) |
| `display_name` | il nome esterno al momento dell'abbinamento, ripulito, al massimo 60 caratteri; solo per l'interfaccia |
| `current` | thread corrente `{ channel, thread_id }` o `null` (vedi §10) |
| `muted_until` | millisecondi Unix o `null` |
| `created_at` | millisecondi Unix |

Si crea **solo** con un codice di abbinamento monouso (§10, §11). La mappa *id del messaggio esterno ↔ messaggio
dell'HUB* (per le risposte) è nello stato dell'istanza, limitata (§6).

## 3. Descrittore

Ogni connettore dichiara nel codice un `ConnectorDescriptor` statico (`&'static`): l'interfaccia è **generica** e si
costruisce solo da qui, senza componenti scritti per un connettore. Il test di conformità lo valida.

| Campo | Contenuto |
|---|---|
| `id` | stabile, `[a-z0-9-]` senza trattini ai bordi (`telegram`, `gmail`); finisce nei file, negli autori dei messaggi e nei log. Non si cambia più |
| `kind` | `source` \| `contact` |
| `label`, `description` | in italiano, per l'interfaccia |
| `version` | intero ≥ 1; cresce quando cambia la forma di `config` o di `filter`, insieme a una migrazione pura (§5) |
| `config_fields` | campi di configurazione dell'istanza |
| `filter_fields` | campi del filtro delle regole (solo `source`, vuoto per `contact`) |
| `actions` | azioni che l'interfaccia mostra come pulsanti: `authorize` (consenso OAuth), `pair` (codice di abbinamento), `test` (verifica credenziali e rete) |
| `permissions` | `hosts`: host di rete ammessi (es. `api.telegram.org`); `hub`: capacità HUB tra `deliver_messages`, `wake_secretaries`, `receive_notices`, `send_as_user` |
| `limits` | facoltativi, più stretti di quelli del runtime: elementi per giro, intervallo minimo di polling, intervallo di polling predefinito (`default_interval_secs`, usato quando la configurazione non ne imposta uno; almeno 60 s e almeno il minimo del descrittore) |

**Campo** (`ConfigField`): `key` (`[a-z0-9_]`, unico), `label`, `help` (testo d'aiuto, può citare un link della
documentazione del servizio), `required`, `kind`:

| `kind` | Valore | Note |
|---|---|---|
| `text` | stringa | `max_len`, `placeholder`; trim |
| `secret` | stringa | **non** sta in `config`: va nel portachiavi (slot = `key`); in lettura solo `set: bool`; mai rimandato al frontend |
| `number` | intero | `min`, `max` |
| `bool` | booleano | `default` |
| `select` | stringa tra `options: [{ value, label }]` | |
| `workspace` | id di un workspace esistente | l'interfaccia mostra i nomi |
| `thread` | `{ channel, thread_id }` di un thread esistente | l'interfaccia mostra canale e titolo |

La validazione generica (tipi, obbligatorietà, limiti, chiavi sconosciute rifiutate) è nel core
(`connectors/descriptor/validate.rs`, puro); quella semantica (es. una query di Gmail non vuota) nel connettore.

**Permessi.** Il runtime li fa rispettare, non il connettore:

| Capacità | Concede | Tipo |
|---|---|---|
| `deliver_messages` | `HubPort::deliver_external` (§4) | `source` |
| `wake_secretaries` | il campo `wake` delle regole diverso da `none`; senza, l'interfaccia non lo offre e la consegna lo ignora | `source` |
| `receive_notices` | ricevere le `HubNotice` in uscita | `contact` |
| `send_as_user` | `HubPort::send_as_user` (§4) | `contact` |

Una capacità non dichiarata manca dal contesto del connettore (il metodo non c'è o risponde «permesso non
dichiarato»): un connettore non può chiedere più di quanto dichiara, e il descrittore è il riassunto revisionabile di
cosa fa.

## 4. Dove girano

**In-process**, come task async su `tauri::async_runtime`: **un supervisore per istanza abilitata**
(`connectors/runtime/supervisor.rs`), che possiede il ciclo di vita della sua istanza (avvio, giri, backoff, stato,
arresto con `JoinHandle::abort`). Nessun processo esterno, nessun runtime in più.

### Contratti (`connectors/contracts/`)

Stessa forma dei contratti degli agenti: trait con oggetti `&'static dyn`, futuri boxati (alias
`BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>`, senza il crate `futures`), nessuna implementazione
di default che nasconda una lacuna.

```rust
pub trait Connector: Send + Sync {
    fn descriptor(&self) -> &'static ConnectorDescriptor;
    /// Pura: config salvata con `from` -> config della `from + 1`; il runtime la chiama passo per passo fino alla `version` corrente.
    fn migrate_config(&self, from: u32, config: Value) -> Result<Value, String>;
    fn validate_config(&self, config: &Value) -> Result<(), String>;
    /// Solo le azioni dichiarate nel descrittore (`authorize`, `pair`, `test`) arrivano qui.
    fn action<'a>(&'a self, ctx: &'a ConnectorContext, action: ActionId) -> BoxFuture<'a, Result<ActionOutcome, ConnectorError>>;
}

pub trait SourceConnector: Connector {
    fn validate_filter(&self, filter: &Value) -> Result<(), String>;
    /// Un giro: legge dal servizio gli elementi nuovi per le regole attive a partire dal cursore
    /// (`None` = da "adesso"). Non consegna: restituisce gli elementi e il cursore nuovo.
    fn poll<'a>(&'a self, ctx: &'a ConnectorContext, routes: &'a [ActiveRoute], cursor: Option<Value>) -> BoxFuture<'a, Result<PollBatch, ConnectorError>>;
}

pub trait ContactConnector: Connector {
    /// Pura: la configurazione chiede di inoltrare le notifiche di questo tipo?
    fn wants_notice(&self, config: &Value, category: NoticeCategory) -> bool;
    /// Long polling (al più `RECEIVE_TIMEOUT`, 70 s): i messaggi in entrata, già tradotti (testo, risposta,
    /// comando, codice di abbinamento, allegati NON scaricati), e il cursore nuovo.
    fn receive<'a>(&'a self, ctx: &'a ConnectorContext, cursor: Option<Value>) -> BoxFuture<'a, Result<ReceiveBatch, ConnectorError>>;
    /// Manda un messaggio a una chat collegata, spezzato al limite del servizio e con la sua frequenza;
    /// ritorna l'id esterno di ogni parte.
    fn send<'a>(&'a self, ctx: &'a ConnectorContext, out: &'a OutboundMessage) -> BoxFuture<'a, Result<Vec<String>, ConnectorError>>;
    /// Scarica un allegato in entrata: il runtime lo chiede solo per un account collegato ed entro i limiti.
    fn fetch_file<'a>(&'a self, ctx: &'a ConnectorContext, file: &'a RemoteFile) -> BoxFuture<'a, Result<IncomingFile, ConnectorError>>;
    /// Un account ha presentato un codice di abbinamento: il connettore risponde come crede (benvenuto, errore generico).
    fn pairing_reply<'a>(&'a self, ctx: &'a ConnectorContext, outcome: &'a PairingOutcome, chat: &'a str) -> BoxFuture<'a, Result<(), ConnectorError>>;
}

/// Un connettore registrato, col suo tipo.
pub enum ConnectorImpl { Source(&'static dyn SourceConnector), Contact(&'static dyn ContactConnector) }
```

`ConnectorError` è la classificazione che il supervisore usa per il backoff (§7): `Transient(String)`,
`RateLimited { retry_after: Option<Duration> }`, `Auth(String)` (→ `needs_auth`), `Config(String)` (configurazione
non valida: si ferma con `error`), `Fatal(String)`. I messaggi sono già ripuliti (mai segreti né contenuti).

Il contesto (`ConnectorContext`, uno solo per fonti, canali di contatto e azioni) dà al connettore solo:

- `config()` già migrata e validata, `secret(slot)` / `set_secret(slot, ..)` limitati ai suoi slot;
- `http()`: un **client HTTP del contesto** (`connectors/runtime/http.rs`) costruito sul `reqwest::Client`
  condiviso, che **rifiuta ogni richiesta verso un host non dichiarato** in `permissions.hosts` (confronto esatto
  dell'host dopo il parsing dell'URL, solo `https`, redirect seguiti solo verso host dichiarati), con timeout per
  chiamata e tetto della dimensione della risposta. Il connettore non costruisce un suo client;
- `state_get(chiave)` / `state_set(chiave, valore)`: i valori privati del connettore nello stato dell'istanza (§6),
  che il runtime salva; cursori, id visti e mappe li gestisce il runtime, non il connettore;
- le regole attive di una fonte e il cursore arrivano come argomenti di `poll`, il cursore di `receive`; il
  codice di abbinamento emesso dal runtime per l'azione `pair` è `pairing_code()` (vale solo in quell'azione);
- `now()` e `random_bytes(n)` iniettati, così la logica del connettore si prova senza rete né orologio;
- l'accesso all'HUB (`deliver`, `send_as_user`, `send_as_user_files`) solo con la capacità dichiarata
  (`deliver_messages`, `send_as_user`): senza, risponde «permesso non dichiarato». Il supervisore consegna da sé gli
  elementi di `poll` e di `receive`; i metodi del contesto servono alle azioni.

La **consegna** nell'HUB non è del connettore: il supervisore prende il `PollBatch` / gli `InboundMessage` e li passa
alla porta verso l'HUB. Così il connettore è un traduttore servizio ↔ DTO, e le garanzie (dedupe, limiti, risvegli,
permessi) stanno in un posto solo.

### La porta verso l'HUB (`hub/ingress.rs`, `hub/send.rs`)

`hub` non dipende da `connectors`. Espone funzioni Rust (e DTO serializzabili, vedi §12) che `connectors` chiama:

| Funzione | Cosa fa |
|---|---|
| `hub::ingress::deliver_external(app, ExternalDelivery) -> Result<DeliveryOutcome, String>` | deposita un elemento esterno in un thread (creandolo se la regola lo dice), con autore `Author::External`, allegati da byte, dedupe e risveglio secondo la regola (§8, §9) |
| `hub::send::send_user_message(app, UserMessage) -> Result<Message, String>` | lo stesso percorso di `hub_send` per un messaggio dell'utente arrivato da un canale di contatto (§8, §10); `UserMessage { channel, thread_id, body, reply_to, attachment_ids, origin }` |
| `hub::send::send_user_files(app, UserMessage, Vec<IncomingFile>) -> Result<Message, String>` | come sopra, con gli allegati come byte (`IncomingFile { name, bytes }`): staging con i limiti dell'app, file saltati raccontati in una riga «Allegati non importati: …» in fondo al testo, staging ripulito se l'invio fallisce |
| `hub::set_notice_sink(app, Arc<dyn NoticeSink>)` | registra un destinatario in più delle notifiche, accanto al frontend (§10); `NoticeSink::deliver(&HubNotice)`, vedi `docs/HUB.MD` «Notifiche» |
| `hub::set_lifecycle_listener(app, Arc<dyn HubLifecycle>)` | **fase 2, costruito**: `HubLifecycle { hub_enabled_changed(app, enabled), workspace_removed(app, ws_id) }`. `hub_enabled_changed` parte da `hub_set_enabled` dopo il salvataggio, solo sulle transizioni vere; `workspace_removed` da `hub::forget_workspace` (chiamata da `workspaces::remove_workspace`) anche con l'HUB spento. Il runtime lo implementa in `connectors/lifecycle.rs` e lo registra `connectors::start`, chiamata da `lib.rs`. Un thread sparito non ha un avviso: lo scopre il giro (regola `existing`, §5) |
| `hub::thread_exists(app, &Channel, thread_id) -> Result<bool, String>` | il thread esiste ancora? Errore con l'HUB spento (non è «no»); un canale sparito risponde `false`. Lo usano i giri delle regole `existing` e i comandi delle regole |

`ExternalDelivery { channel, target: DeliveryTarget, author, body, files: Vec<IncomingFile>, origin: MessageOrigin, wake: ExternalWake }`,
con `DeliveryTarget::{ Existing(thread_id), New { title } }`, `ExternalWake::{ None, Owner, Secretary(nome) }` e
`DeliveryOutcome { thread_id, message_id, duplicate }`: sono tipi Rust, non DTO del frontend. Il comportamento
completo (dedupe, thread chiuso riaperto, testo troncato, risveglio) è in `docs/HUB.MD`, «API interna».

`hub_send` è un adattatore sottile su una funzione condivisa, `hub::send::send_user_message(app, UserMessage)`
(salvataggio, `deliver_if_answer`, `SecretaryEngine::on_message`), usata anche dai canali di contatto (**fatto in
fase 1**), senza aver cambiato il comportamento di `hub_send`.

Le registrazioni le fa `lib.rs` in `setup`, come `hub::set_board_source` e `hub::set_board_actions`: il livello di
composizione conosce entrambi i moduli.

### Livelli e dipendenze

```text
 4  automation  mcp  connectors   orchestrazione / shim MCP / connettori esterni dell'HUB
 2  board  workspaces  hub        dominio
 0  platform  validate  store  secrets  ...
```

| Modulo | Dipende da | Non dipende da |
|---|---|---|
| `connectors` | `hub` (API pubblica: `ingress`, `send`, `attachments`, i DTO del modello, `HubNotice`, `HubLifecycle`, `thread_exists`, `Channel`), `store`, `secrets`, `workspaces` (solo `WorkspaceStore`, per sapere se il canale di una regola esiste); crate `reqwest` (solo `runtime/http.rs`) e `tokio` (`time`, `sync`, `macros`; `net` e `io-util` solo per il listener loopback del consenso OAuth) | `automation`, `agents`, `board`, `ipc`, `pty`, `mcp` |
| `hub` | come oggi | `connectors` (mai: solo sink e listener registrati da `lib.rs`) |

**Regola architetturale** (test in `connectors/architecture.rs`, come `agents/architecture.rs`): fuori da
`connectors/<id>/` nessun file nomina un connettore specifico; solo `connectors/registry.rs` lo fa. I connettori non
importano `hub::` se non `hub::ingress` e i tipi pubblici del modello, e non usano `reqwest` direttamente (solo il
client del contesto). Le cartelle del nucleo (`commands`, `contracts`, `descriptor`, `files`, `model`, `registry`,
`runtime`, `testing`) non sono connettori: la lista `CORE_FOLDERS` e la lista dei connettori noti (`KNOWN`) stanno in
`connectors/architecture.rs`. Lato frontend `tests/architecture.test.mjs` verifica che nessun componente nomini un
connettore.

### Struttura dei file

```text
src-tauri/src/connectors.rs            modulo, load_from_disk / start / stop_all (chiamate da lib.rs)
src-tauri/src/connectors/
  contracts/        Connector, SourceConnector, ContactConnector, ConnectorImpl, ConnectorError, DTO, ConnectorContext (+ HubPort, Clock)
  descriptor/       ConnectorDescriptor, campi, permessi; validate.rs (valori di config e filtro, puro); validate/check.rs (descrittore, solo test)
  registry.rs       CONNECTORS: l'unico punto che nomina i connettori (+ registry/test_registry.rs per i test)
  model.rs          ConnectorInstance, ConnectorRoute, ConnectorLink, ConnectorsFile (DTO ts-rs, prefisso `Connector`)
  ids.rs, limits.rs  generazione e validazione degli id; i tetti
  files.rs          connectors.json e connector_state/ su store/ (decoder tollerante)
  state.rs          InstanceState: cursori, id visti, mappe, valori privati; tutto limitato (puro)
  secrets.rs        i segreti di un'istanza nel portachiavi (conti `connector-<istanza>-<slot>`)
  lifecycle.rs      il runtime come ascoltatore di `HubLifecycle`
  runtime/          manager.rs (ConnectorsRuntime), supervisor.rs, round.rs (un giro di una fonte), contact.rs e
                    contact_loop.rs (canali di contatto), notices.rs (uscita), pairing.rs, health.rs, backoff.rs (puro),
                    http.rs, hub_port.rs, store.rs, migrate.rs, time.rs
  commands/         comandi Tauri: instances.rs, actions.rs, routes.rs; checks.rs (verifiche pure), view.rs (DTO)
  conformance.rs    test di conformità comuni a ogni connettore registrato
  architecture.rs   test architetturali
  testing.rs        connettori finti, HUB finto, orologio finto (solo test)
  telegram/         connettore di riferimento `contact` (fase 4)
  gmail/            connettore di riferimento `source` (fase 5)
src-tauri/src/hub/ingress.rs, hub/send.rs, hub/lifecycle.rs   deliver_external, send_user_message, send_user_files, HubLifecycle (lato HUB)
```

## 5. Ciclo di vita

| Stato / passo | Cosa succede |
|---|---|
| **Disponibile** | il connettore è nel registro (compilato nell'app). Compare in Impostazioni › Connettori con descrizione e permessi, senza istanze |
| **Configurare** | «Aggiungi» crea un'istanza **disabilitata** con etichetta e config validata; i campi `secret` vanno nel portachiavi; le azioni (`authorize`, `test`, `pair`) sono disponibili |
| **Abilitare** | il runtime avvia il supervisore (solo con l'HUB acceso): stato `starting` → `ok`. Alla **prima** abilitazione i cursori partono da *adesso*: gli elementi arretrati non entrano (come gli eventi della board, che non si ricostruiscono; vedi "Domande aperte") |
| **Disabilitare** | il supervisore si ferma (abort dopo il giro in corso, al più il timeout di una chiamata); config, segreti, stato e regole restano. Riabilitata riprende dai cursori |
| **Aggiornare** | avviene con l'app. Se la `version` del descrittore è maggiore di `config_version`, al caricamento si applica `migrate_config` (pura, un passo per versione, provata nei test di conformità) e la config migrata si salva al primo salvataggio; una migrazione che fallisce lascia l'istanza ferma con stato `error` e la config originale intatta. Anche i `filter` delle regole migrano con la stessa `version` |
| **Rimuovere** | con conferma: si ferma il supervisore, si cancellano l'istanza, le sue regole o i suoi collegamenti, il file di stato e **tutti** i suoi slot del portachiavi. Un errore del portachiavi va nel log e nell'interfaccia, ma l'istanza sparisce comunque (nessun segreto resta orfano senza che l'utente lo sappia) |

Altri eventi:

- **HUB spento** (`hub_set_enabled(false)`): tutti i supervisori si fermano (via `hub::set_lifecycle_listener`) e le
  notifiche in attesa dei canali di contatto si buttano, come `HubNotifier::discard`. Le impostazioni dei connettori
  si leggono e si scrivono comunque. Riacceso, ripartono le istanze abilitate.
- **Workspace rimosso**: `hub::forget_workspace` avvisa il listener; il runtime toglie le regole con quel canale,
  le voci di `per_conversation` dei suoi thread e i `current` dei collegamenti che puntavano lì (un messaggio dal
  canale di contatto dice che il thread non c'è più).
- **Thread sparito** (potato o del canale rimosso): una regola `existing` che lo punta si **disabilita da sola** con
  il motivo («thread non più disponibile»), visibile nell'interfaccia; le altre regole proseguono. Una voce di
  `per_conversation` che punta a un thread sparito si scarta e l'elemento apre un thread nuovo.
- **App chiusa**: nessun connettore gira (niente servizi di sistema né processi in background). Alla riapertura,
  con l'HUB acceso, le istanze abilitate riprendono **dai cursori**: gli elementi arrivati nel frattempo entrano
  (per Gmail entro il tetto per giro, nei giri successivi; per Telegram finché il servizio li conserva, 24 ore).

## 6. File

| File | Contenuto |
|---|---|
| `<app_data>/connectors.json` | `{ instances: [...], routes: [...], links: [...] }` nella busta di `store/` (`JsonFile` statico, catena di migrazioni in `connectors/files.rs`) |
| `<app_data>/connector_state/<istanza>.json` | stato del connettore per l'istanza (`JsonFile::named`): cursori, insieme limitato di id già visti, mappe (`per_conversation`, id esterni ↔ messaggi). Forma propria del connettore, versionata con la sua `version` |

Mai nella repository dell'utente; mai segreti in nessuno dei due. Come per l'HUB, con i connettori mai configurati
non si crea nessun file. Tetti (costanti in `connectors/runtime/`):

| Limite | Valore |
|---|---|
| Istanze | 20 |
| Regole per istanza | 50 |
| Collegamenti per istanza | 5 |
| Id già visti per istanza (dedupe) | 5000, i più vecchi escono per primi |
| Voci di `per_conversation` per istanza (chiave di conversazione → thread) | 2000, le meno recenti escono (un elemento successivo della conversazione apre un thread nuovo) |
| Mappe id esterno ↔ messaggio HUB per istanza | 2000 voci ciascuna (le due direzioni), le meno recenti escono |
| Intervallo di polling di una fonte | dalla config (`interval_secs` o `interval_minutes`, se il descrittore dichiara il campo), altrimenti `default_interval_secs` del descrittore; almeno 60 s |
| Elementi per giro | 20 (`more` → il giro dopo parte subito) |
| Valori privati del connettore nello stato | 64 chiavi, 64 KB ciascuno |

Un file di stato corrotto segue `store/` (backup, avviso `store-warning`) e l'istanza riparte dai cursori di
default, cioè da *adesso*: si perdono al più gli elementi del periodo, mai si duplica in massa.

## 7. Affidabilità

**Almeno una volta + dedupe = effettivamente una volta.** Il supervisore, a ogni giro:

1. chiede gli elementi (`poll` / `receive`);
2. li consegna uno alla volta (`deliver_external` / `send_user_message` / `send_user_files`); la consegna salva il
   thread, e solo allora l'id entra nell'insieme dei visti (e, per una regola `per_conversation`, la conversazione
   nella mappa) dello stato in memoria;
3. se tutto il lotto è consegnato, il cursore nuovo entra nello stato;
4. lo stato si salva su disco **dopo ogni giro**, anche fallito (errore di consegna o di poll, giro scaduto per
   `ROUND_TIMEOUT`, regola disabilitata per `THREAD_GONE`) e dopo un lotto di un canale di contatto fallito a metà:
   il cursore resta il vecchio, ma i visti e le mappe delle consegne già riuscite sono su disco.

Così un'uscita dall'app durante il backoff non fa ripetere nulla: al riavvio il vecchio cursore ripresenta gli
elementi, e la dedupe per `(regola, external_id)` scarta quelli già consegnati. Se l'app muore tra 2 e 4 (o il file
di stato è stato perso) regge la dedupe dell'HUB per origine, cioè `(instance, external_id)` di `Message.origin`:
`deliver_external` cerca un messaggio con la stessa origine nel thread di destinazione o, per un thread nuovo
(`new_per_item`, il primo elemento di una conversazione), in tutto il canale, e lo restituisce con `duplicate: true`
senza creare thread né svegliare nessuno; `send_user_message` / `send_user_files` (canali di contatto) non
riscrivono né svegliano per un messaggio con un'origine già presente nel thread e restituiscono quello salvato. È
una rete di sicurezza, non una garanzia assoluta: un messaggio potato (`MESSAGES_MAX`, thread chiusi oltre il
tetto) non si trova più.

**Backoff** (`connectors/runtime/backoff.rs`, puro, casualità iniettata): esponenziale con jitter, da 5 s a 10 min,
azzerato dal primo giro riuscito.

| Errore | Effetto |
|---|---|
| rete, timeout, 5xx (`Transient`) | stato `backoff`, prossimo tentativo secondo il backoff |
| 429 (`RateLimited`) | si rispetta `Retry-After` (secondi o data HTTP), mai meno del backoff corrente, al massimo 1 ora |
| 401 / 403 (`Auth`) | stato `needs_auth`: il supervisore **si ferma** finché l'utente non rifà `authorize` o cambia il segreto (nessun tentativo a vuoto che bruci limiti o blocchi l'account) |
| config non valida (`Config`), `Fatal` | stato `error`, supervisore fermo fino a una modifica dell'istanza |

**Tetti per giro**: ogni chiamata HTTP ha un timeout (30 s; per il long polling il timeout del servizio + 10 s), l'intero
`poll` di una fonte un tetto (90 s, `POLL_TIMEOUT`: sono più chiamate), il giro intero un tetto (2 min) oltre il quale il supervisore lo abbandona; al più **20 elementi** per giro (gli altri
al giro successivo, senza attesa) e **al più un risveglio per thread per giro**, dall'ultimo elemento che lo chiede:
come gli eventi della board, per contenere i costi del modello (il contesto porta anche i messaggi precedenti).

**Stato di salute** (`ConnectorHealth`, in memoria, non su disco):

| Stato | Significato |
|---|---|
| `disabled` | istanza disabilitata, o HUB spento |
| `starting` | supervisore avviato, primo giro in corso |
| `ok` | ultimo giro riuscito (`last_ok_at`) |
| `backoff` | errore transitorio, riprova a `next_retry_at` |
| `needs_auth` | credenziali rifiutate: serve l'utente |
| `error` | fermo per configurazione, migrazione o errore non recuperabile |
| `unavailable` | il connettore dell'istanza non è nel registro di questa versione dell'app |

Il DTO è `ConnectorStatus { instance, state, last_ok_at, last_error, next_retry_at }`, con `last_error` **ripulito**:
un testo fisso scelto dalla classe dell'errore (il tipo `&'static str` impedisce che vi arrivino corpi di risposta,
URL con token o contenuti). Evento `connector-status` con l'**elenco completo** a ogni cambiamento; comando
`connector_status` per la lettura iniziale.

**Epoca di esecuzione.** Ogni avvio di un'istanza apre un'epoca (`StatusRegistry::begin_run`); fermare l'istanza
(disabilitazione, eliminazione, HUB spento, riavvio) la invalida. Solo l'esecuzione corrente scrive lo stato di
salute: un task interrotto ma ancora in volo non può sovrascrivere il `disabled` deciso dal manager con un «ok»
tardivo.

## Comandi

I comandi Tauri (`connectors/commands/`) validano gli argomenti per primi (`checks.rs`, puro), fanno l'I/O in
`crate::blocking`, rispondono con errori in italiano e non restituiscono né registrano mai un segreto. Funzionano
anche con l'HUB spento (salvo la verifica di un thread esistente, che lo richiede).

| Comando | Cosa fa |
|---|---|
| `connector_list_descriptors()` | i descrittori dei connettori compilati |
| `connector_list_instances()` | `ConnectorInstanceView { instance, secrets: [{ slot, set }], status }`: mai i valori dei segreti |
| `connector_status()` | l'elenco completo dei `ConnectorStatus` (lo stesso dell'evento) |
| `connector_create_instance(connector, label, config)` | crea un'istanza **disabilitata**: etichetta, config validata (generica contro i campi del descrittore e semantica con `validate_config`); i campi `secret` nella config sono rifiutati; al massimo 20 istanze |
| `connector_update_instance(id, label, config)` | rivalida e salva; se la config cambia l'istanza riparte (esce da `needs_auth` e da `error`). Una config salvata con un'altra versione del descrittore non si modifica |
| `connector_delete_instance(id)` | ferma il task, toglie istanza, regole e collegamenti, cancella il file di stato e tutti gli slot del portachiavi; un errore del portachiavi o del file di stato non annulla l'eliminazione e torna in `ConnectorDeleteOutcome.warning` |
| `connector_set_enabled(id, enabled)` | abilita o disabilita; un connettore non disponibile non si abilita |
| `connector_set_secret(id, slot, value)` | lo slot deve essere dichiarato dal descrittore e il valore valido; se l'istanza era in `needs_auth` riparte |
| `connector_clear_secret(id, slot)` | toglie il segreto di uno slot dichiarato |
| `connector_test(id)` | esegue l'azione `test`, se dichiarata |
| `connector_run_action(id, action)` | solo le azioni dichiarate (`authorize`, `pair`, `test`); esito `ConnectorActionOutcome { message, detail, pairing_code }`. Con `pair` il runtime emette il codice monouso (`issue_pairing_code`), lo mette nel contesto (`ConnectorContext::pairing_code()`) e lo restituisce; richiede l'istanza in esecuzione. Tetto di 5 minuti. Un errore del connettore arriva come messaggio generico della sua classe |
| `connector_list_routes(channel, threadId)` | le regole, filtrabili per canale e per thread (quelle che puntano a quel thread esistente) |
| `connector_create_route(instance, route)` | `route: ConnectorRouteInput { label, filter, channel, thread, wake, enabled }`. L'istanza deve essere una fonte; al massimo 50 regole per istanza; il canale deve essere Generale o un workspace esistente; un thread `existing` deve esistere nell'HUB; `wake` diverso da `none` richiede la capacità `wake_secretaries`; il filtro è validato contro i campi del descrittore e con `validate_filter` |
| `connector_update_route(id, route)` | come la creazione; il thread si cerca di nuovo solo se cambia thread o canale o la regola si riabilita. Una regola riabilitata o ricollocata perde l'errore «thread non più disponibile» |
| `connector_delete_route(id)` | elimina la regola |
| `connector_list_links(instance)` | gli account esterni collegati a un'istanza di contatto |
| `connector_delete_link(id)` | revoca un collegamento |

Evento `connector-status`: `ConnectorStatus[]`. Lato frontend: `api.connector*` in `src/lib/tauri.ts`, lo store
`src/stores/connectors.ts` (descrittori, istanze, stati) e `app/actions/connectors.ts`; l'interfaccia generica è in `components/settings/ConnectorsPage.tsx` e `components/hub/LinkedSourcesDialog.tsx` (fase 3).

**Log**: solo id dell'istanza, classe dell'errore, conteggi («gmail con-…: 3 elementi consegnati»). Mai segreti,
indirizzi, oggetti, testi o nomi di allegati. Il token del bot Telegram sta nell'URL della Bot API: il client del
contesto non registra mai l'URL e ripulisce gli errori di reqwest (che lo includono) prima di restituirli.

## 8. Eventi e risvegli unificati

Un solo concetto di risveglio con un **motivo**. `WakeCause` (`hub/engine/sessions.rs`) cresce:

```rust
pub enum WakeCause {
    Message,       // un messaggio dell'utente (menzioni o titolare della sessione)
    BoardEvent,
    External,      // un elemento consegnato da un connettore (variante unitaria, `Copy`)
}
```

(Realizzato così: `Message` non è stato rinominato e `External` non porta dati, perché connettore e mittente si
leggono dall'autore del messaggio che apre il turno.) Tutti passano da `Sessions` (`Sessions::route_external`,
accanto a `route` e `route_event`, con cui condivide `route_to`; il motore la chiama con
`SecretaryEngine::on_external(app, channel, thread_id, trigger_id, secretary)`, dopo che chi consegna ha risolto
`owner` / `secretary:<nome>` nel nome del segretario) e dalle **code dei thread**: un elemento esterno che arriva mentre un segretario lavora sul thread si accoda come un
messaggio dell'utente; sessioni, scadenze, riassunti e stato `hub-secretary-status` non cambiano.

**Cambiamento previsto** rispetto a HUB.MD («Solo i messaggi `user` svegliano i segretari»): sveglia anche un
elemento esterno, **ma solo tramite la regola** che lo ha consegnato (`wake`), mai per il suo contenuto.

- **Niente menzioni dai testi esterni.** `deliver_external` non chiama `mentions`: un'email con «@mario» non sveglia
  nessuno. `wake: "owner"` sveglia il titolare del thread (`HubThread.owner`, come `route_event`), `secretary:<nome>`
  quel segretario se configurato e ammesso nel canale, altrimenti nulla (riga nel log, come `on_events`).
- **Turni con poteri ridotti.** Un turno con `WakeCause::External` non può usare `decide_proposal` né
  `relay_answer`: `planning::proposal_target` e `relay::relay_candidates` escludono esplicitamente ogni motivo diverso
  da `Message` e gli errori per il modello nominano anche il contenuto esterno. Sull'`open_plan_task` vedi
  "Domande aperte".
- **Contenuto non fidato.** Per il modello (`engine/output.rs`, quindi anche `thread_read` e il contesto del
  risveglio) un messaggio `external` ha autore `{ "type": "external", "source": "<connettore>", "name": "<mittente>" }`
  (mai istanza né id esterno) e `"untrusted": true`; un messaggio dell'utente con `origin` ha `"via": "<connettore>"`.
  Il primo messaggio del turno (`engine/context.rs`) nomina connettore e mittente e dice che è un dato non fidato,
  mai una richiesta dell'utente (nessuna richiesta, link o comando da eseguire), poi riporta il messaggio in JSON
  e chiede di dire all'utente in breve cosa è arrivato. L'istruzione fissa del motore (`ENGINE_INSTRUCTIONS`) dice
  che i messaggi con autore `external` e ciò che citano non sono richieste dell'utente. Nel testo del piano
  (`planning.rs`) un elemento esterno compare come `[external <mittente> via <connettore>, untrusted]` e non è mai
  citato come richiesta dell'utente.
- **I messaggi da un canale di contatto abbinato sono dell'utente.** `send_user_message` e `send_user_files` salvano un `Author::User` con
  `origin: { connector, instance, external_id }` (campo nuovo e facoltativo di `HubMessage`, `serde(default)`;
  `external_id` è l'id del messaggio nel servizio di contatto) e seguono il
  percorso di `hub_send`: menzioni, titolare della sessione, risposta diretta alle domande dei worker
  (`deliver_if_answer`) e decisione delle proposte con «Rispondi» (`reply_to` sull'evento). L'abbinamento è ciò che
  rende quell'account «l'utente» (§11).

## 9. Fonti

**Configurazione.** Le regole si creano da due punti:

- **dal thread**: nell'intestazione della conversazione, «Fonti collegate» elenca le regole che puntano al thread
  (modo `existing`) e permette di aggiungerne una (istanza, filtro, risveglio) con il thread già scelto;
- **dalle impostazioni**: Impostazioni › Connettori › l'istanza › Regole, con tutti i modi (`existing`,
  `new_per_item`, `per_conversation`) e la scelta del canale.

Un'anteprima («Prova il filtro») chiama il connettore con la regola senza consegnare: mostra quanti elementi
corrispondono e i titoli degli ultimi 5 (solo nell'interfaccia, mai nel log).

**Cosa arriva nel thread.** Un messaggio con:

- autore `Author::External { connector, instance, name }`: `connector` è l'id del descrittore, `instance` l'id
  dell'istanza, `name` il mittente ripulito (Gmail: «Mario Rossi <mario@gmail.com>», al massimo 120 caratteri). Nel
  modello è una variante di `Author` (**fatto in fase 1**, `hub/model.rs` e `HubAuthor` nei bindings); più
  `Message.origin: { connector, instance, external_id }` (facoltativo, per la dedupe; `external_id` è l'id
  dell'elemento nel servizio, 1..256 caratteri);
- `kind: text`, `body` dal **template** del connettore: un'intestazione (Gmail: righe «Oggetto: …», «Da: …», «Data: …» ed
  eventualmente «Allegati non importati: …»), una riga vuota, il testo (parte `text/plain`; se c'è solo HTML, una conversione in testo semplice
  pura e provata nel connettore, senza dipendenze nuove), ripulito dai caratteri di controllo e tagliato a
  `BODY_MAX` (8000) con «… (testo troncato)»;
- **allegati** entro i limiti dell'HUB: al più `ATTACHMENTS_MAX` (5), ciascuno al più `ATTACHMENT_SIZE_MAX` (10 MB),
  scritti con `stage_bytes` (nome ripulito, MIME dalle firme) e spostati nella cartella del thread come quelli
  dell'utente. Quelli in più o troppo grandi **non** si scaricano: il testo termina con «Allegati non importati:
  nome (dimensione), …». **Cambiamento previsto**: oggi `add_message_with_attachments` accetta solo messaggi
  `user`, accetterà anche `external`; gli allegati restano invisibili nel contenuto ai segretari (solo nome, tipo e
  dimensione) e leggibili da pianificatore e worker con i percorsi, come oggi.

Il thread di destinazione: `existing` → il thread della regola; `new_per_item` → un thread nuovo intitolato con il
titolo dell'elemento (l'oggetto, tagliato a `TITLE_MAX`); `per_conversation` → il thread della mappa, o uno nuovo
se la chiave è nuova. Un elemento in un thread chiuso **lo riapre**, come gli eventi di un task (vedi "Domande
aperte").

**Notifica.** Un `NoticeKind` nuovo, `External { connector }`: titolo «Nuovo da <etichetta dell'istanza>», suono
`ding`, significatività subito sotto `Reply`. Passa dal raggruppamento esistente: un'email seguita dalla risposta
del segretario svegliato dà una notifica sola (la risposta).

**Risveglio** secondo `wake` (§8), dopo il salvataggio e fuori dal lock dei thread, come `on_events`.

## 10. Canali di contatto

**Abbinamento.** In Impostazioni › Connettori › l'istanza, «Abbina un account» (azione `pair`) genera un **codice
monouso** (12 caratteri casuali da `getrandom`, alfabeto senza ambiguità, valido 10 minuti, tenuto solo in memoria)
e lo mostra con le istruzioni del connettore (Telegram: un link `https://t.me/<bot>?start=<codice>` e il testo
`/start <codice>`). Il primo messaggio con il codice valido, da una **chat privata**, crea il `ContactLink` e
consuma il codice; il bot risponde con il benvenuto e l'aiuto (`/help`). Il nome mostrato del collegamento è quello
del mittente (`InboundMessage.sender_name`, ripulito). Un codice sbagliato o scaduto risponde in modo generico e conta
per il limite di frequenza (§11). Il collegamento si revoca dall'interfaccia o con `/unlink` (alias `/stop`) dalla
chat.

**Chi fa cosa.** Il connettore traduce: riconosce comandi, codici di abbinamento, risposte e allegati secondo la
sintassi del servizio, spezza i testi al limite del servizio e ne rispetta la frequenza. Il runtime
(`connectors/runtime/contact/`) decide: dedupe, abbinamento, collegamenti, thread corrente, esecuzione dei comandi e
testi delle risposte (`replies.rs`, uguali per ogni canale di contatto), limiti degli allegati, mappa delle risposte.
Un account non collegato non arriva mai all'HUB e i suoi allegati non si scaricano.

**Notifiche in uscita.** Le stesse notifiche dell'HUB, dopo lo **stesso raggruppamento** di `HubNotifier`
(finestra di 3 s, una per thread): a fine finestra `hub/notify/dispatch.rs` consegna la notizia a ogni destinatario
registrato (`hub::set_notice_sink`) oltre a emettere `hub-notify`. Ogni destinatario ha il suo interruttore: il
frontend continua a guardare `notifications`; un canale di contatto guarda `notices.all` dell'istanza, i **tipi** scelti
nella sua configurazione (`ContactConnector::wants_notice`: Telegram ha un interruttore per tipo nei campi del
descrittore) e `muted_until` del collegamento, **indipendenti** da `notifications`. Il DTO verso i contatti
(`ContactNotice`) porta, oltre a `HubNotice` (con l'id del messaggio dell'HUB che l'ha causata), il tipo
(`NoticeCategory`), i nomi di canale e thread e il **testo completo** del messaggio (una risposta del segretario va
letta intera fuori dall'app, non in 120 caratteri). Il testo inviato è `«<canale> › <titolo del thread>»`, a capo, il
testo (o il titolo della notifica se il messaggio è vuoto), spezzato dal connettore al limite del servizio (Telegram
4096, al massimo 3 parti). Ogni parte inviata entra nella mappa id esterno → `(canale, thread, messaggio HUB)`
(`runtime/contact/refs.rs`, valore codificato nello stato dell'istanza).

**Messaggi in entrata.** Da un collegamento: vanno al **thread corrente** del collegamento con `send_user_message` (o
`send_user_files` se hanno allegati), con `origin { connector, instance, external_id }`: sono messaggi dell'utente,
quindi menzioni, titolare della sessione e risposte dirette funzionano come nell'app. Senza thread corrente il bot
spiega come sceglierne uno e non salva nulla; un thread corrente sparito si azzera con un avviso. Un errore dell'HUB
(testo troppo lungo, thread chiuso al limite…) si racconta nella chat e il messaggio non si ritenta.

**Risposte.** Rispondere (la funzione «rispondi» del servizio) a un messaggio del bot = messaggio **nello stesso
thread** di quella notifica, con `reply_to` sul messaggio dell'HUB che l'ha causata (dalla mappa), e quel thread
diventa il corrente. Quindi rispondere alla notifica di una domanda di un worker è la **risposta diretta**
(`deliver_if_answer`), rispondere alla notifica di una proposta permette al segretario di **deciderla**
(`decide_proposal`), esattamente come «Rispondi» nell'HUB. Una risposta a un messaggio del bot non più nella mappa
va al thread corrente senza `reply_to`, con un avviso.

**Comandi** (in italiano nelle risposte; nomi in inglese):

| Comando | Effetto |
|---|---|
| `/help` | elenco dei comandi (anche `/start` senza codice; un comando sconosciuto risponde con l'aiuto) |
| `/threads` | i thread aperti di tutti i canali (prima quelli che aspettano l'utente, `waiting_user`, poi i più recenti), numerati, al massimo 10, ciascuno con il nome del canale |
| `/use <n>` | rende corrente il thread n dell'ultimo elenco (l'elenco sta in memoria: dopo un riavvio va rifatto `/threads`) |
| `/ws [n\|nome]` | senza argomento elenca i canali (Generale e workspace per **nome**, mai per id); con un numero o un nome sceglie il canale di `/new` (`ConnectorLink.channel`) |
| `/new <titolo>` | crea un thread nel canale scelto con `/ws` (altrimenti quello del thread corrente, altrimenti Generale) e lo rende corrente |
| `/status` | riepilogo: thread corrente, thread che aspettano l'utente (in tutto e per canale), notifiche silenziate o no |
| `/mute [minuti]` | silenzia le notifiche del collegamento: senza argomento fino a `/unmute`, con un numero per quei minuti (al più 7 giorni) |
| `/unmute` | riattiva le notifiche |
| `/unlink` (`/stop`) | revoca il collegamento |

**Allegati in entrata**: documenti e foto della chat (di una foto la dimensione più grande) si scaricano con
`fetch_file` entro `ATTACHMENT_SIZE_MAX` e `ATTACHMENTS_MAX` per messaggio (Telegram raggruppa le foto in album: un
album nello stesso lotto = un messaggio dell'HUB) e passano da `send_user_files`; quelli oltre i limiti o non
scaricabili si saltano con un avviso nella chat. Sticker, vocali, video e simili senza testo si ignorano (riga nel
log). **In uscita** v1 non manda file.

**Dedupe**: per id esterno del messaggio (`<chat>:<message_id>` per Telegram) nell'insieme dei visti, oltre al
cursore (`offset` = ultimo `update_id` + 1), che avanza solo dopo l'elaborazione di tutto il lotto; i visti si salvano
anche dopo un lotto fallito a metà, e l'HUB scarta un messaggio con un'origine già presente nel thread (§7).

**Limite in entrata**: al più 20 messaggi al minuto per account collegato; gli altri si scartano, con un solo avviso
per finestra.

**Messaggi modificati**: ignorati in v1 (il bot non risponde; il testo nell'HUB resta quello originale).

**Un account esterno ↔ un utente.** L'app è di un solo utente: ogni collegamento è quell'utente. Più collegamenti
per istanza sono ammessi (telefono e tablet con account diversi), ciascuno con il suo thread corrente; più thread si
gestiscono con `/threads`, `/use` e le risposte alle notifiche.

## 11. Sicurezza

- **Codice fidato.** I connettori sono codice compilato nell'app, revisionato come il resto: in v1 nessun codice di
  terzi si carica a runtime. Il descrittore (host e capacità) è la superficie da revisionare.
- **Nessun panico su dati esterni.** Con `panic = "abort"` in release un panico chiude l'app: niente `unwrap`,
  `expect`, indicizzazioni o tagli di stringa non controllati su dati che vengono dalla rete; ogni parsing è
  `Result`. I test di conformità passano a ogni connettore input malformati, vuoti, enormi e unicode.
- **Segreti solo nel portachiavi**: mai in `connectors.json`, nello stato, nei log, nei DTO, negli errori o negli
  eventi; il frontend vede solo `set: bool`. I segreti si validano (ASCII visibile, lunghezza) prima di salvarli,
  come l'API key del provider.
- **Allowlist degli host** nel client del contesto (§4): solo `https`, solo host dichiarati, redirect controllati,
  dimensione delle risposte limitata (10 MB + margine per gli allegati, 1 MB per le altre).
- **Abbinamento esplicito e revocabile** dei canali di contatto: solo chat private, codice monouso a scadenza; i
  messaggi di chat o account **non abbinati** si ignorano (nessuna risposta oltre al messaggio generico per un
  `/start` non valido, nessun salvataggio). Il bot non va aggiunto a gruppi: se succede, esce dal gruppo.
- **Limiti di frequenza**: in entrata al più 20 messaggi al minuto per collegamento (gli altri si scartano con un
  avviso); tentativi di abbinamento al più 5 per 10 minuti per istanza, poi i codici in corso si annullano; in
  uscita i limiti del servizio (§13).
- **Contenuti esterni non fidati** per i segretari (§8) e niente menzioni dai loro testi; nessun contenuto esterno
  finisce in una riga di comando, in un percorso (i nomi degli allegati passano da `name::clean`) o nell'HTML del
  frontend senza escape (il frontend mostra i messaggi come testo, come oggi).
- **Blocchi e fallimenti.** Un connettore che si blocca non blocca l'app: ogni chiamata ha un timeout, il giro un
  tetto, e il supervisore abbandona il giro (abort del task) e passa in `backoff`. Un connettore che fallisce
  ripetutamente resta in `backoff` (al più 10 min tra i tentativi) o si ferma in `needs_auth`/`error`. Nessun lock
  dell'HUB è tenuto durante una chiamata di rete: la consegna prende il lock dei thread solo per salvare, come gli
  eventi della board.

## 12. Alternative valutate e raccomandazione

| | Approccio | Pro | Contro | Decisione |
|---|---|---|---|---|
| **A** | Connettori **compilati** dietro contratti Rust, con registro, conformance e test architetturali | lo stesso modello già collaudato degli agenti; tipi, test e revisione; nessun runtime in più; accesso diretto a portachiavi, `store/` e HUB | un connettore nuovo richiede una release dell'app | **scelta per ora** |
| **B** | Plugin **esterni come processi sidecar** che parlano JSON-RPC sul canale IPC locale già esistente (`src-tauri/src/ipc/`), con un token per istanza come `CLI_COMPANION_TOKEN` per i tab | connettori fuori dall'app, in qualunque linguaggio, isolati in un processo | distribuzione, aggiornamento e fiducia del codice di terzi; ciclo di vita dei processi | **fase successiva**: i DTO di consegna e notifica (`ExternalDelivery`, `UserSend`, `ContactNotice`, `InboundMessage`, `OutboundMessage`) sono serializzabili apposta perché un adattatore `SidecarConnector` implementi gli stessi contratti, con il descrittore letto da un manifest del processo |
| **C** | Plugin **solo-manifest** dichiarativi (URL, mapping JSON) | nessun codice | insufficienti per OAuth, long polling, paginazione, allegati binari, backoff specifici | scartato |
| **D** | **Sandbox WASM** (wasmtime) | il miglior isolamento per codice di terzi | dipendenza pesante, ABI e host functions da progettare | futuro, dopo B se servirà codice non fidato |
| — | Plugin **Node** | ecosistema ampio | l'app non distribuisce un runtime Node | scartato |

**Webhook.** Un'app desktop non ha un URL pubblico: niente webhook. Si usa il **polling** o il **long polling**:
Telegram `getUpdates`; Gmail con l'API in polling. Gmail push richiede Google Cloud Pub/Sub con un endpoint pubblico:
scartato. **Discord** richiede il gateway WebSocket (una dipendenza nuova, es. un client WebSocket su tokio): fase
successiva, con lo stesso contratto `ContactConnector`.

## 13. Implementazioni di riferimento

### Telegram (`connectors/telegram/`, `contact`)

**Creare il bot e abbinarlo** (guida per l'utente):

1. In Telegram apri una chat con **@BotFather**, scrivi `/newbot`, scegli un nome e uno username che finisca con
   `bot` (es. `mio_hub_bot`). BotFather risponde con il **token** (`123456789:AA…`): è una password, non condividerlo.
2. Nell'app: Impostazioni › Connettori › Telegram › «Aggiungi», incolla il token nel campo «Token del bot» (va nel
   portachiavi del sistema, mai su disco) e scegli quali notifiche inoltrare. «Prova» chiama `getMe` e mostra il
   nome del bot.
3. Abilita l'istanza (serve l'HUB acceso) e premi «Abbina un account»: l'app mostra un codice valido 10 minuti e il
   link `https://t.me/<bot>?start=<codice>`. Apri il link dal telefono (o scrivi al bot `/start <codice>`): il bot
   risponde con il benvenuto e l'aiuto.
4. Scegli dove scrivere con `/threads` e `/use <n>` (o `/ws` e `/new <titolo>`), oppure rispondi direttamente alle
   notifiche. Il collegamento si revoca dalle impostazioni o con `/unlink`.

Il bot va usato solo in chat privata: aggiunto a un gruppo esce da solo. Se il bot aveva un webhook, il primo avvio
lo toglie (webhook e `getUpdates` si escludono).

**Codice** (`connectors/telegram/`):

- **Configurazione**: un campo `secret` `bot_token` (formato `<cifre>:<almeno 30 caratteri A-Za-z0-9_->` controllato
  prima di ogni chiamata) e otto interruttori `bool` per i tipi di notifica: risposta di un segretario, domanda di un
  worker, task fallito, task che richiede attenzione, proposta, pianificazione senza proposta, thread concluso (tutti
  accesi), task o revisione finiti (spento). Gli elementi delle fonti (`External`) non si inoltrano in v1. Host:
  `api.telegram.org`. Capacità: `receive_notices`, `send_as_user`. Azioni: `test` (`getMe`, mostra il nome del bot),
  `pair` (link `t.me` con il codice del runtime).
- **Token**: nel percorso degli URL della Bot API (`/bot<token>/<metodo>`, `/file/bot<token>/<percorso>`): mai nei
  log, negli errori (testi fissi, al più il codice d'errore), nei DTO, nello stato o nei messaggi.
- **Ricezione**: long polling `getUpdates` (`timeout` 50 s, richiesta HTTP entro 60 s, sotto il `RECEIVE_TIMEOUT` del
  runtime) con `offset` = ultimo `update_id` + 1 (cursore salvato dal runtime dopo il lotto),
  `allowed_updates: ["message", "my_chat_member"]`. Al primo avvio (nessun cursore) `deleteWebhook` e si parte da
  *adesso* (`offset: -1`). Il servizio conserva gli update non letti per 24 ore. Il parsing (`updates.rs`, puro) non
  va mai in panico: un update inatteso si ignora con una riga nel log (solo il tipo).
- **Abbinamento e comandi**: `/start <codice>` in chat privata (`chat.type == "private"`; il payload ammette
  `A-Za-z0-9_-` fino a 64 caratteri); `/nome[@bot] argomenti` → `ContactCommand` (`commands.rs`, puro).
  `my_chat_member` verso un gruppo → `leaveChat`. Messaggi di gruppi e canali, modificati, sticker: ignorati.
- **Invio**: `sendMessage` in testo semplice (nessun `parse_mode`: niente escape di Markdown/HTML da sbagliare), senza
  anteprima dei link, parti di al più **4096 unità UTF-16** tagliate a fine riga o parola, al massimo 3
  (`text.rs`, puro); `reply_parameters` sulla prima parte per legare la risposta. Frequenza: **~1 messaggio al
  secondo per chat** (`pacer.rs`, puro); un 429 diventa `RateLimited` con il `retry_after` del corpo.
- **Errori**: 401/404 → `Auth` (token rifiutato: `needs_auth`); 429 → `RateLimited`; 409 (webhook o un altro client)
  e ogni altro errore, compreso 403 di un utente che ha bloccato il bot → `Transient`.
- **Allegati**: `getFile` + download da `api.telegram.org/file/bot…` (percorso validato, risposta limitata a
  `RESPONSE_MAX_FILES`; la Bot API scarica al più 20 MB, il limite dell'HUB è più stretto).
- **Messaggi**: id esterno `<chat>:<message_id>`; `message.reply_to_message` → `<chat>:<id>` → mappa → `reply_to`.

### Gmail (`connectors/gmail/`, `source`)

**Configurare Gmail** (guida per l'utente):

1. Su [console.cloud.google.com](https://console.cloud.google.com) crea un progetto (è il tuo: nessuna chiave è
   inclusa nell'app).
2. API e servizi › Libreria › **Gmail API** › Abilita.
3. Configura la schermata di consenso OAuth: tipo di utente «Esterno»; aggiungi il tuo indirizzo come utente di
   test solo finché provi. Poi porta lo stato di pubblicazione su **«In production»**: in «Testing» i refresh token
   scadono dopo 7 giorni e l'istanza passa a «Serve una nuova autorizzazione». Per un uso personale l'app può
   restare non verificata (al consenso Google mostra un avviso; `gmail.readonly` è uno scope con restrizioni).
4. Credenziali › Crea credenziali › ID client OAuth › tipo di applicazione **App desktop**. Copia l'ID client e il
   client secret.
5. Nell'app: Impostazioni › Connettori › Gmail › «Aggiungi», incolla ID client e client secret (il secret va nel
   portachiavi del sistema), scegli l'intervallo di controllo in minuti (predefinito 2) e premi «Autorizza»: si apre
   il browser sul consenso di Google; dopo averlo accettato la pagina dice che puoi chiuderla e «Prova» mostra il
   tuo indirizzo.
6. Abilita l'istanza (serve l'HUB acceso).
7. Crea la regola dal thread: intestazione della conversazione › «Fonti collegate» › aggiungi: istanza, query
   (sintassi di ricerca di Gmail, es. `from:mario@gmail.com`) e risveglio (nessuno / il proprietario / un
   segretario). Entrano solo le email arrivate **dopo** la creazione della regola.

**Codice** (`connectors/gmail/`):

- **Configurazione**: `client_id` (`text`), `interval_minutes` (`number` 1..60, predefinito 2 tramite
  `default_interval_secs`), segreto `client_secret`, slot interno `refresh_token`. Host: `accounts.google.com`,
  `oauth2.googleapis.com`, `gmail.googleapis.com`. Capacità: `deliver_messages`, `wake_secretaries`. Filtro:
  `query`, 1..500 caratteri, parentesi e virgolette bilanciate. Azioni: `authorize` e `test`
  (`users/me/profile`, mostra l'indirizzo).
- **`authorize`**: OAuth 2.0 per app installate. Un listener su **127.0.0.1** con porta scelta dal SO (redirect
  `http://127.0.0.1:<porta>`), `state` casuale, **PKCE** `S256`, consenso aperto con `tauri-plugin-opener`, scope
  **`gmail.readonly`**, `access_type=offline`, `prompt=consent`. Si accetta solo la prima richiesta con `code` o
  `error` sul percorso `/` (gli altri percorsi ricevono 404); uno `state` sbagliato è rifiutato e l'autorizzazione
  fallisce. Il listener si chiude dopo quel ritorno o dopo l'attesa di **240 s** (sotto il tetto di 5 minuti delle azioni, per lasciare il tempo allo scambio del codice e al profilo); ogni connessione ha 10 s per mandare la richiesta. Il
  codice si scambia con il verifier; il **refresh token** va nel portachiavi, l'access token resta solo in memoria
  (per istanza) e si rinnova prima della scadenza.
- **Errori**: 400 `invalid_grant` al rinnovo → `Auth` → `needs_auth` («Serve una nuova autorizzazione»); rete e 5xx
  → backoff; 429 e 403 di limite di frequenza → `RateLimited`; 403 con l'API non abilitata → `Config`.
- **Polling** (nessun controllo preliminare con `getProfile`/`history`): per ogni regola `users.messages.list` con
  `q = "(<query>) after:<secondi>"`. I secondi vengono dal cursore della regola: `floor_ms` (quando la regola è stata
  letta la prima volta: nulla di più vecchio entra mai) e `last_ms` (il più recente consegnato) meno un margine di
  10 minuti per il ritardo di indicizzazione della ricerca, mai sotto `floor_ms`. Gli id già letti vicino al
  cursore stanno in un insieme `recent` limitato (200 per regola) per non riscaricarli; la dedupe delle consegne è
  del runtime (insieme dei visti e `origin.external_id` = id del messaggio Gmail). Il cursore lo salva il runtime
  solo dopo la consegna. Al più **20 email per giro** su tutte le regole (a turno fra le regole, dalla più vecchia);
  il resto al giro dopo subito (`more`). Una regola appena nata non elenca nulla nel suo primo giro (crea solo la voce del cursore). Si leggono al più 5 pagine da 100 id per regola e giro. Un budget per giro limita i byte degli allegati (**50 MB**) e il tempo (**60 s**, misurato con l'orologio del contesto): esaurito, il giro si ferma e il successivo riparte subito; si elabora sempre almeno un messaggio. Un messaggio che più regole intercettano si legge e si scarica una volta sola (ogni regola ha il suo elemento). Un 401 invalida il token in cache e si riprova una volta; un secondo 401 è `Auth`. Tetti delle risposte: 1 MB (elenchi, profilo), 5 MB (messaggio), circa 13,4 MB (allegato: base64 di 10 MB più la busta JSON, oltre i 10,25 MB di `RESPONSE_MAX_FILES`).
- **Messaggio**: autore `External { connector: "gmail", instance, name: "Nome <indirizzo>" }` (120 caratteri); corpo
  con «Oggetto: …», «Da: …», «Data: …», se serve «Allegati non importati: nome (dimensione), …», una riga vuota e il
  testo (`text/plain`, o `text/html` ridotto a testo da un convertitore puro), tagliato dall'HUB a 8000 caratteri.
  Allegati: fino a 5 da 10 MB, scaricati con `messages.attachments.get` e consegnati; gli altri elencati per nome.
  `new_per_item`: titolo del thread = oggetto tagliato a 120; `per_conversation`: la chiave di conversazione è il
  `threadId` di Gmail.
- **Sicurezza**: il contenuto delle email non è fidato: nessuna menzione interpretata, il risveglio lo decide solo
  la regola. Mai nei log: segreti, token, indirizzi, oggetti, testi, nomi degli allegati (solo id dell'istanza,
  classe dell'errore, conteggi).

## 14. Fasi

| Fase | Contenuto | Fatto quando |
|---|---|---|
| **1. Fondamenta HUB** (consegnata) | `Author::External`, `origin` e `external_id` dei messaggi (ts-rs, compatibilità in lettura), allegati da byte per `external`; `hub::send::send_user_message` e `send_user_files` (con `hub_send` adattatore), `hub::ingress::deliver_external`; `WakeCause::ExternalItem` e `Sessions::route_external`, contenuto non fidato nel contesto; `NoticeKind::External`, destinatari multipli in `HubNotifier` (`set_notice_sink`, `message_id` in `HubNotice`); resa dei messaggi esterni nella conversazione. **`set_lifecycle_listener` non è in questa fase: passa alla fase 2** | test unitari dei nuovi percorsi, `hub_send` invariato |
| **2. Runtime e registro** (consegnata) | `hub::set_lifecycle_listener` (HUB spento o acceso, workspace rimosso); `connectors/`: contratti, descrittore e validazione, registro, `connectors.json` e `connector_state/`, supervisori, backoff, salute (`connector-status`), client HTTP con allowlist, segreti, abbinamento, comandi Tauri; un connettore finto nei test, conformance e test architetturali | un connettore finto consegna e riceve end-to-end nei test |
| **3. Interfaccia generica** (consegnata, tranne i collegamenti) | Impostazioni › Connettori (`ConnectorsPage`: disponibili, istanze, stato in tempo reale, azioni, segreti in sola scrittura), form dei campi dal descrittore (`lib/connectorForm.ts`), regole delle fonti (`lib/connectorRoutes.ts`, editor nelle impostazioni), «Fonti collegate» nell'intestazione del thread; resta da fare l'elenco dei collegamenti dei canali di contatto e l'elenco dei permessi del descrittore | nessun componente nomina un connettore |
| **4. Telegram** (consegnata) | connettore `contact` di riferimento; nel runtime: `ContactNotice` con tipo e testo completo, filtro per tipo (`wants_notice`), comandi e testi delle risposte, mappa delle risposte con canale e thread, allegati scaricati solo per gli account collegati (`fetch_file`), `HubPort::{channels, open_threads, create_thread}` (`hub::contact`), `ConnectorLink.channel` | abbinamento, notifiche, risposte, comandi, allegati |
| **5. Gmail** (consegnata) | connettore `source` di riferimento | OAuth, regole, consegna, risveglio |
| poi | Discord (gateway WebSocket), adattatore sidecar (B), altre fonti (calendario, RSS, issue tracker…) | |

Alla fine della fase 2 la ricetta «Aggiungere un connettore» è in `CONTRIBUTING.md` e la guida in
[wiki/integrare-un-connettore.md](wiki/integrare-un-connettore.md), sul modello di
[wiki/integrare-un-coding-agent.md](wiki/integrare-un-coding-agent.md).

**Ancora rinviato** (dalla fase 2):

- gli interruttori per tipo di `ConnectorNotices` (`notices.replies`, `questions`, …) restano inutilizzati: il filtro
  per tipo dei canali di contatto sta nella configurazione del connettore (`wants_notice`, dai campi del
  descrittore), così l'interfaccia generica lo mostra senza componenti dedicati; conta solo `notices.all`;
- i test dei **timeout** delle singole chiamate (30 s) e del giro (2 min): `with_timeout` e il tetto del giro sono nel
  supervisore ma coperti solo dai test di logica con l'attesa finta, non da test che scadono davvero.

## Domande aperte

1. **Arretrato alla prima abilitazione**: partire da *adesso* (proposta) o offrire «importa gli ultimi N elementi»
   (es. 10) per regola?
2. **`open_plan_task` nei turni da elemento esterno**: escluderlo per default (proposta, per la prompt injection: il
   piano partirebbe da un testo non fidato) con un'opzione per regola che lo abilita, oppure lasciarlo come per gli
   eventi della board?
3. **Thread chiuso**: un elemento esterno lo riapre (proposta, come gli eventi dei task) o lo lascia chiuso?
4. **Notifiche verso i canali di contatto**: sempre (proposta), o solo quando la finestra dell'app non è in primo
   piano, come il frontend?
5. **Notifiche delle fonti**: ogni elemento consegnato notifica (proposta, con `NoticeKind::External`), o solo quelli
   che non svegliano un segretario (la risposta notificherà comunque)?
6. **Eco dei propri messaggi**: un messaggio scritto nell'HUB dall'app deve comparire anche nel canale di contatto
   (proposta: no in v1)?
7. **Messaggi modificati** nei canali di contatto: ignorarli (proposta v1) o aggiungere una nota «messaggio
   modificato» nel thread?
8. **Gmail**: basta `gmail.readonly` (proposta) o serve in futuro rispondere alle email (`gmail.send`, cioè un
   connettore anche `contact` verso l'esterno, fuori dal perimetro di v1)?
