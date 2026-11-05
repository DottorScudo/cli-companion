# Connettori dell'Agents HUB

Documento di progetto (nessun codice ancora): i **connettori** estendono l'[Agents HUB](HUB.MD) verso l'esterno.
È il riferimento dei task che li realizzeranno (vedi "Fasi"). Dove una decisione cambia un comportamento descritto in
HUB.MD lo si dice esplicitamente come **cambiamento previsto**; finché la fase che lo introduce non è consegnata vale
HUB.MD.

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

La logica di questi tre passi oggi vive **dentro il comando Tauri**: non c'è una funzione Rust riusabile da chi non
è il frontend.

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

### Cosa manca

| Manca | Perché serve |
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
- `getrandom` (già usato da `automation/auth.rs`) per i codici di abbinamento e lo stato OAuth.
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
| `connector` | id di un connettore del registro; un id sconosciuto (connettore tolto da una versione successiva) lascia l'istanza su disco, ferma, con stato `error` «connettore non disponibile» |
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
| `label` | facoltativa, 0..60 caratteri |
| `filter` | oggetto JSON validato dal connettore (`validate_filter`) contro i campi del filtro del descrittore (Gmail: `{ "query": "from:mario@gmail.com" }`) |
| `channel` | `general` o l'id di un workspace esistente (`Channel`) |
| `thread` | `{ "mode": "existing", "thread_id": "thr-…" }` (un thread scelto dall'utente; gli elementi vanno tutti lì) \| `{ "mode": "new_per_item" }` (un thread nuovo per elemento, intitolato con il titolo dell'elemento) \| `{ "mode": "per_conversation" }` (la chiave di conversazione esterna dell'elemento, es. il `threadId` di Gmail, sceglie il thread; la mappa chiave → thread la tiene il connettore nel suo stato, vedi §6) |
| `wake` | `"none"` (solo messaggio nel thread) \| `"owner"` (il titolare del thread, se c'è) \| `"secretary:<nome>"` (un segretario preciso, che deve essere configurato e ammesso nel canale al momento della consegna) |
| `enabled` | `true` alla creazione |

Un elemento che soddisfa più regole della stessa istanza si consegna **una volta per regola** (thread diversi sono
destinazioni diverse); due regole che puntano allo stesso thread lo ricevono una volta sola (dedupe per thread, §7).

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
| `limits` | facoltativi, più stretti di quelli del runtime: elementi per giro, intervallo minimo di polling |

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
    /// Pura: config salvata con `from` -> config della `version` corrente.
    fn migrate_config(&self, from: u32, config: Value) -> Result<Value, String>;
    fn validate_config(&self, config: &Value) -> Result<(), String>;
    fn action<'a>(&'a self, ctx: &'a ActionContext, action: ActionId) -> BoxFuture<'a, Result<ActionOutcome, ConnectorError>>;
}

pub trait SourceConnector: Connector {
    fn validate_filter(&self, filter: &Value) -> Result<(), String>;
    /// Un giro: legge dal servizio gli elementi nuovi per le regole abilitate,
    /// partendo dai cursori dello stato. Non consegna: restituisce.
    fn poll<'a>(&'a self, ctx: &'a SourceContext, routes: &'a [ActiveRoute]) -> BoxFuture<'a, Result<PollBatch, ConnectorError>>;
}

pub trait ContactConnector: Connector {
    /// Long polling: aspetta (con timeout) i messaggi in entrata.
    fn receive<'a>(&'a self, ctx: &'a ContactContext) -> BoxFuture<'a, Result<Vec<InboundMessage>, ConnectorError>>;
    /// Manda un messaggio a una chat collegata; ritorna l'id esterno.
    fn send<'a>(&'a self, ctx: &'a ContactContext, out: &'a OutboundMessage) -> BoxFuture<'a, Result<String, ConnectorError>>;
}
```

`ConnectorError` è la classificazione che il supervisore usa per il backoff (§7): `Transient(String)`,
`RateLimited { retry_after: Option<Duration> }`, `Auth(String)` (→ `needs_auth`), `Config(String)` (configurazione
non valida: si ferma con `error`), `Fatal(String)`. I messaggi sono già ripuliti (mai segreti né contenuti).

I contesti (`SourceContext`, `ContactContext`, `ActionContext`) danno al connettore solo:

- `config()` già migrata e validata, `secret(slot)` / `set_secret(slot, ..)` limitati ai suoi slot;
- `http()`: un **client HTTP del contesto** (`connectors/runtime/http.rs`) costruito sul `reqwest::Client`
  condiviso, che **rifiuta ogni richiesta verso un host non dichiarato** in `permissions.hosts` (confronto esatto
  dell'host dopo il parsing dell'URL, solo `https`, redirect seguiti solo verso host dichiarati), con timeout per
  chiamata e tetto della dimensione della risposta. Il connettore non costruisce un suo client;
- `state()`: lo stato persistente dell'istanza (cursori, id visti, mappe, §6), salvato dal runtime;
- per le fonti, le regole attive; per i contatti, i collegamenti e i codici di abbinamento in corso;
- `now()` e `random()` iniettati, così la logica del connettore si prova senza rete né orologio.

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
| `hub::set_lifecycle_listener(listener)` | **non costruito in fase 1, resta alla fase 2**: avvisa quando l'HUB si spegne o si accende e quando un workspace o un thread spariscono, così il runtime ferma i supervisori e toglie le regole |

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
| `connectors` | `hub` (solo `hub::ingress`, i DTO del modello e `HubNotice`), `store`, `secrets`, `workspaces` (nomi dei workspace nei comandi e nei comandi `/ws`), `validate`; crate `reqwest` | `automation`, `agents`, `board`, `ipc`, `pty`, `mcp` |
| `hub` | come oggi | `connectors` (mai: solo sink e listener registrati da `lib.rs`) |

**Regola architetturale** (test in `connectors/architecture.rs`, come `agents/architecture.rs`): fuori da
`connectors/<id>/` nessun file nomina un connettore specifico; solo `connectors/registry.rs` lo fa. I connettori non
importano `hub::` se non `hub::ingress` e i tipi pubblici del modello, e non usano `reqwest` direttamente (solo il
client del contesto). Lato frontend `tests/architecture.test.mjs` verificherà che nessun componente nomini `telegram`
o `gmail`.

### Struttura dei file prevista

```text
src-tauri/src/connectors.rs            modulo, stati gestiti, registrazione dei comandi
src-tauri/src/connectors/
  contracts/        Connector, SourceConnector, ContactConnector, ConnectorError, DTO dei contesti
  descriptor/       ConnectorDescriptor, campi, permessi; validate.rs (puro)
  registry.rs       CONNECTORS: l'unico punto che nomina i connettori
  model.rs          ConnectorInstance, ConnectorRoute, ContactLink (DTO ts-rs, prefisso `Connector`)
  files.rs          connectors.json e connector_state/ su store/
  runtime/          supervisor.rs, backoff.rs (puro), health.rs, http.rs, notices.rs (uscita), pairing.rs
  commands.rs       comandi Tauri
  conformance.rs    test di conformità comuni a ogni connettore
  architecture.rs   test architetturali
  telegram/         connettore di riferimento `contact`
  gmail/            connettore di riferimento `source`
src-tauri/src/hub/ingress.rs, hub/send.rs   deliver_external, send_user_message, send_user_files (lato HUB)
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
| Id già visti per istanza (dedupe) | 2000, i più vecchi escono per primi |
| Voci di `per_conversation` per regola | 1000, le meno recenti escono (un elemento successivo della conversazione apre un thread nuovo) |
| Mappa id esterno ↔ messaggio HUB per collegamento | 500 |

Un file di stato corrotto segue `store/` (backup, avviso `store-warning`) e l'istanza riparte dai cursori di
default, cioè da *adesso*: si perdono al più gli elementi del periodo, mai si duplica in massa.

## 7. Affidabilità

**Almeno una volta + dedupe = effettivamente una volta.** Il supervisore, a ogni giro:

1. chiede gli elementi (`poll` / `receive`);
2. li consegna uno alla volta (`deliver_external` / `send_user_message` / `send_user_files`); la consegna salva il thread;
3. solo dopo salva il cursore e aggiunge gli id all'insieme dei visti.

Se l'app si chiude tra 2 e 3, al giro successivo gli stessi elementi tornano: la dedupe per `(istanza,
external_id)` (e per regola) li scarta. `deliver_external` controlla anche nel thread di destinazione (un
messaggio con la stessa `origin`, cioè `instance` ed `external_id`), così la dedupe regge anche se il file di stato è stato perso.

**Backoff** (`connectors/runtime/backoff.rs`, puro, casualità iniettata): esponenziale con jitter, da 5 s a 10 min,
azzerato dal primo giro riuscito.

| Errore | Effetto |
|---|---|
| rete, timeout, 5xx (`Transient`) | stato `backoff`, prossimo tentativo secondo il backoff |
| 429 (`RateLimited`) | si rispetta `Retry-After` (secondi o data HTTP), mai meno del backoff corrente, al massimo 1 ora |
| 401 / 403 (`Auth`) | stato `needs_auth`: il supervisore **si ferma** finché l'utente non rifà `authorize` o cambia il segreto (nessun tentativo a vuoto che bruci limiti o blocchi l'account) |
| config non valida (`Config`), `Fatal` | stato `error`, supervisore fermo fino a una modifica dell'istanza |

**Tetti per giro**: ogni chiamata HTTP ha un timeout (30 s; per il long polling il timeout del servizio + 10 s), il
giro intero un tetto (2 min) oltre il quale il supervisore lo abbandona; al più **20 elementi** per giro (gli altri
al giro successivo, senza attesa) e **al più un risveglio per thread per giro**, dall'ultimo elemento che lo chiede:
come gli eventi della board, per contenere i costi del modello (il contesto porta anche i messaggi precedenti).

**Stato di salute** (`ConnectorHealth`, in memoria, non su disco):

| Stato | Significato |
|---|---|
| `disabled` | istanza disabilitata, o HUB spento |
| `starting` | supervisore avviato, primo giro in corso |
| `ok` | ultimo giro riuscito (`last_ok_at`) |
| `backoff` | errore transitorio, riprova a `retry_at` |
| `needs_auth` | credenziali rifiutate: serve l'utente |
| `error` | fermo per configurazione, migrazione o errore non recuperabile |

Con `last_error` **ripulito** (classe dell'errore e messaggio generico in italiano, mai corpi di risposta, URL con
token o contenuti). Evento `connector-status` con `{ instance, health, last_error, last_ok_at, retry_at }` a ogni
cambiamento; comando `connector_status` per la lettura iniziale.

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
  `relay_answer`: `planning::proposal_target` e `relay::relay_target` escludono esplicitamente ogni motivo diverso
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
- `kind: text`, `body` dal **template** del connettore: una riga di intestazione (Gmail: «Email da <mittente> —
  <oggetto>»), una riga vuota, il testo (parte `text/plain`; se c'è solo HTML, una conversione in testo semplice
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
consuma il codice; il bot risponde con il benvenuto e `/help`. Un codice sbagliato o scaduto risponde in modo
generico e conta per il limite di frequenza (§11). Il collegamento si revoca dall'interfaccia (e con `/stop` dalla
chat).

**Notifiche in uscita.** Le stesse notifiche dell'HUB, dopo lo **stesso raggruppamento** di `HubNotifier`
(finestra di 3 s, una per thread): **cambiamento previsto** di `hub/notify/dispatch.rs`, che a fine finestra
consegna la notizia a ogni destinatario registrato (`hub::set_notice_sink`) invece di emettere solo `hub-notify`.
Ogni destinatario ha il suo interruttore: il frontend continua a guardare `notifications`; un canale di contatto
guarda `notices` dell'istanza (generale + per tipo: risposte, domande, fallimenti, attenzione, proposte, conclusioni,
fonti) e `muted_until` del collegamento, **indipendenti** da `notifications`. Il DTO verso i contatti
(`ContactNotice`) porta, oltre a `HubNotice`, l'id del messaggio dell'HUB che l'ha causata e il suo **testo
completo** (una risposta del segretario va letta intera fuori dall'app, non in 120 caratteri), tagliato al limite del
servizio (Telegram 4096, spezzato in più messaggi se serve, al massimo 3). Il messaggio inviato riporta canale e
titolo del thread; il suo id esterno entra nella mappa id esterno ↔ messaggio HUB.

**Messaggi in entrata.** Da un collegamento: vanno al **thread corrente** del collegamento con `send_user_message` (o `send_user_files` se hanno allegati). Senza
thread corrente il bot risponde con l'elenco (`/threads`) e non salva nulla.

**Risposte.** Rispondere (la funzione «rispondi» del servizio) a un messaggio del bot = messaggio **nello stesso
thread** di quella notifica, con `reply_to` sul messaggio dell'HUB che l'ha causata (dalla mappa), e quel thread
diventa il corrente. Quindi rispondere alla notifica di una domanda di un worker è la **risposta diretta**
(`deliver_if_answer`), rispondere alla notifica di una proposta permette al segretario di **deciderla**
(`decide_proposal`), esattamente come «Rispondi» nell'HUB. Una risposta a un messaggio del bot non più nella mappa
va al thread corrente senza `reply_to`, con un avviso.

**Comandi** (in italiano nelle risposte; nomi in inglese):

| Comando | Effetto |
|---|---|
| `/help` | elenco dei comandi |
| `/threads` | i thread aperti del canale corrente (prima quelli che aspettano l'utente, `waiting_user`), numerati, al massimo 10 |
| `/use <n>` | rende corrente il thread n dell'ultimo elenco |
| `/ws [nome]` | senza argomento elenca i canali (Generale e workspace per **nome**, mai per id); con un nome cambia canale |
| `/new <titolo>` | crea un thread nel canale corrente (`create_thread`) e lo rende corrente |
| `/status` | riepilogo: thread corrente, thread che aspettano l'utente per canale, segretari svegli |
| `/mute [minuti\|off]` | silenzia le notifiche del collegamento (default 60 minuti) o le riattiva |
| `/stop` | revoca il collegamento |

**Allegati in entrata**: documenti e foto della chat si scaricano entro `ATTACHMENT_SIZE_MAX` e `ATTACHMENTS_MAX` per
messaggio (Telegram raggruppa le foto in album: un album = un messaggio dell'HUB), con `stage_bytes`; quelli oltre i
limiti si rifiutano con un avviso nella chat. **In uscita** v1 non manda file: la notifica dice «2 allegati
nell'HUB».

**Dedupe**: per `update_id` (Telegram) nell'insieme dei visti, oltre all'offset.

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

- **Configurazione**: un campo `secret` `bot_token` (creato dall'utente con @BotFather; l'aiuto lo spiega). Host:
  `api.telegram.org`. Capacità: `receive_notices`, `send_as_user`. Azioni: `test` (`getMe`, mostra il nome del bot),
  `pair`.
- **Ricezione**: long polling `getUpdates` con `offset` = ultimo `update_id` + 1 (salvato nello stato dopo la
  consegna), `timeout` 50 s, `allowed_updates: ["message", "my_chat_member"]`. All'abilitazione `deleteWebhook` (i due modi si
  escludono). Il servizio conserva gli update non letti per 24 ore.
- **Abbinamento**: `/start <codice>` in chat privata (`chat.type == "private"`), deep link `t.me/<bot>?start=<codice>`
  (il payload di `/start` ammette `A-Za-z0-9_-` fino a 64 caratteri). `my_chat_member` verso un gruppo → `leaveChat`.
- **Invio**: `sendMessage` in testo semplice (nessun `parse_mode`: niente escape di Markdown/HTML da sbagliare),
  limite **4096 caratteri** per messaggio; `reply_parameters` per legare la risposta. Frequenza: **~1 messaggio al
  secondo per chat** (e 30 al secondo in tutto per il bot): una coda d'uscita per collegamento, e un 429 con
  `retry_after` dal corpo della risposta.
- **Allegati**: `getFile` + download da `api.telegram.org/file/bot…` (la Bot API scarica al più 20 MB, il limite
  dell'HUB è più stretto).
- **Messaggi**: `message.reply_to_message.message_id` → mappa → `reply_to`; `edited_message` non si richiede.

### Gmail (`connectors/gmail/`, `source`)

- **Credenziali**: OAuth 2.0 per **app installate**. L'utente crea nel **suo** progetto Google Cloud un client OAuth
  di tipo **Desktop** e abilita la Gmail API; nella configurazione dell'istanza inserisce `client_id` (`text`) e
  `client_secret` (`secret`). L'aiuto spiega i passi. Host: `oauth2.googleapis.com`, `gmail.googleapis.com`
  (`accounts.google.com` solo come URL aperto nel browser). Capacità: `deliver_messages`, `wake_secretaries`.
- **`authorize`**: un listener HTTP temporaneo su **127.0.0.1** con porta scelta dal SO (redirect loopback
  `http://127.0.0.1:<porta>`), **PKCE** `S256` e `state` casuale (`getrandom`), consenso aperto nel browser con
  `tauri-plugin-opener`, scope **`gmail.readonly`**, `access_type=offline`, `prompt=consent`. Il listener accetta una
  sola richiesta con lo `state` giusto, poi chiude (al più 5 minuti). Il **refresh token** va nel portachiavi
  (slot `oauth_refresh_token`); l'access token resta in memoria fino alla scadenza.
- **Avvertenza**: con la schermata di consenso del progetto in stato **«Testing»** i refresh token **scadono dopo 7
  giorni** (l'istanza passa a `needs_auth`): va messa **«In production»**. Per un uso personale l'app può restare
  non verificata (Google mostra un avviso al consenso); `gmail.readonly` è uno scope con restrizioni. L'interfaccia
  e l'aiuto lo dicono.
- **Filtro**: `{ "query": "<sintassi di ricerca di Gmail>" }`, es. `from:mario@gmail.com`, `label:clienti
  has:attachment`; 1..500 caratteri.
- **Polling**: ogni 60 s (configurabile 30..3600, `number`). Un controllo economico per giro: `users.getProfile`
  (o `users.history.list` dallo `historyId` salvato) dice se è cambiato qualcosa; se sì, per ogni regola
  `users.messages.list` con `q = "<query> after:<secondi>"` dal cursore della regola, poi `users.messages.get` dei
  nuovi dal più vecchio, entro il tetto di 20 per giro. Cursore per regola = `internalDate` dell'ultimo consegnato
  (con margine) + insieme dei visti per gli id allo stesso istante.
- **Elemento**: `external_id` = id del messaggio Gmail, chiave di conversazione = `threadId` (per
  `per_conversation`), titolo = oggetto, mittente = `From`, testo dalla parte `text/plain`, allegati da
  `users.messages.attachments.get`, link `https://mail.google.com/mail/u/0/#all/<id>` in fondo al testo.
- **Errori**: 401 → rinnovo dell'access token; `invalid_grant` al rinnovo → `needs_auth`; 403
  `rateLimitExceeded`/429 → `RateLimited`.

## 14. Fasi

| Fase | Contenuto | Fatto quando |
|---|---|---|
| **1. Fondamenta HUB** | `Author::External`, `origin` e `external_id` dei messaggi (ts-rs, compatibilità in lettura), allegati da byte per `external`; `hub::send::send_user_message` e `send_user_files` (con `hub_send` adattatore), `hub::ingress::deliver_external`; `WakeCause::ExternalItem` e `Sessions::route_external`, contenuto non fidato nel contesto; `NoticeKind::External`, destinatari multipli in `HubNotifier` (`set_notice_sink`, `message_id` in `HubNotice`); resa dei messaggi esterni nella conversazione. **`set_lifecycle_listener` non è in questa fase: passa alla fase 2** | test unitari dei nuovi percorsi, `hub_send` invariato |
| **2. Runtime e registro** | `hub::set_lifecycle_listener` (HUB spento o acceso, workspace o thread spariti); `connectors/`: contratti, descrittore e validazione, registro, `connectors.json` e `connector_state/`, supervisori, backoff, salute (`connector-status`), client HTTP con allowlist, segreti, abbinamento, comandi Tauri; un connettore finto nei test, conformance e test architetturali | un connettore finto consegna e riceve end-to-end nei test |
| **3. Interfaccia generica** | Impostazioni › Connettori (disponibili, istanze, stato, azioni, permessi), editor delle regole, «Fonti collegate» nel thread, collegamenti dei canali di contatto, tutto dal descrittore | nessun componente nomina un connettore |
| **4. Telegram** | connettore `contact` di riferimento | abbinamento, notifiche, risposte, comandi, allegati |
| **5. Gmail** | connettore `source` di riferimento | OAuth, regole, consegna, risveglio |
| poi | Discord (gateway WebSocket), adattatore sidecar (B), altre fonti (calendario, RSS, issue tracker…) | |

Alla fine della fase 2 una ricetta «Aggiungere un connettore» entra in `CONTRIBUTING.md` e una guida in `docs/wiki/`,
sul modello di [wiki/integrare-un-coding-agent.md](wiki/integrare-un-coding-agent.md).

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
