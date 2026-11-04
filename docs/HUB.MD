# Agents HUB

L'**Agents HUB** è una modalità opzionale di cli-companion: uno spazio stile Slack con un **canale per
workspace** più un canale **Generale**, e dentro ogni canale dei **thread** di messaggi. Nel backend ci sono
l'interruttore globale, il modello dati, la persistenza, i comandi Tauri, l'evento verso il frontend e gli
**eventi della board** nei thread (vedi "Eventi della board") e la **configurazione dei segretari** (provider del
modello, lingua, sessioni, elenco dei segretari, API key: vedi "Segretari e provider"); nel frontend la vista (vedi
"Interfaccia"), le **impostazioni dei segretari** e la loro presenza nella conversazione (menzioni, "sta scrivendo",
chi è sveglio) e il **motore dei segretari**, che li fa rispondere nei thread (vedi "Motore dei segretari"), aprire
task di pianificazione e decidere le proposte quando l'utente risponde al loro evento (vedi "Pianificazione dal
thread").

Il codice sta in `src-tauri/src/hub.rs` e `src-tauri/src/hub/`. Le dipendenze e il posto del modulo nella mappa
sono in [DESIGN.MD](DESIGN.MD), "`hub/`".

## Interruttore

`hub_settings.json` nella cartella dati dell'app: `enabled` è `false` di default, nella busta di `store/`
(`{schema_version, data}`, prima versione con `adopt_envelope`). La struttura ha `#[serde(default)]`: i campi
della tappa 2 (`provider`, `language`, `session`, `secretaries`, vedi "Segretari e provider") si sono aggiunti
senza migrazione e un file della tappa 1 (`{ "enabled": true }`) si legge senza perdere `enabled`.

Con l'HUB **spento** (default):

- non si crea nessun file (né `hub_settings.json` finché l'utente non cambia l'impostazione, né `threads/`);
- i comandi di scrittura (`hub_create_thread`, `hub_send`) rispondono «L'Agents HUB è spento: attivalo nelle
  impostazioni.»;
- le letture (`hub_list_threads`, `hub_get_thread`) funzionano e non creano file;
- gli eventi della board non si registrano: nessuna istantanea in memoria, nessuna scrittura;
- board, task e il resto dell'app non cambiano. Rispegnerlo non tocca i thread su disco;
- le impostazioni (provider, API key, lingua, sessioni, segretari) si leggono e si scrivono comunque: la pagina
  delle impostazioni le mostra anche a HUB spento.

## Segretari e provider

I **segretari** sono bot con un nome che l'utente sveglia con `@nome` in un thread e che parlano con un modello
tramite un endpoint compatibile OpenAI. Qui la **configurazione** (codice in `hub/settings/` e
`hub/api_key.rs`); il motore che li fa parlare (vedi "Motore dei segretari") legge questi dati. Tutto è in `hub_settings.json` tranne la API key.

| Campo | Default | Regole |
|---|---|---|
| `provider.base_url` | `""` | un solo endpoint per tutta l'app: `http://` o `https://` (es. `http://127.0.0.1:8080/v1` per llama-server, `http://127.0.0.1:11434/v1` per Ollama), un host, porta facoltativa 1..65535, niente credenziali (`user@`), frammento, spazi o caratteri di controllo, al massimo 2048 caratteri; i `/` finali si tolgono. Vuoto = non configurato |
| `provider.model` | `""` | al massimo 128 caratteri dopo il trim, niente caratteri di controllo. Vuoto = non configurato |
| `language` | `it` | lingua delle risposte dei segretari: `it`, `en`, `es`, `fr`, `de` (`HubLanguage`). Il motore aggiunge al system prompt "Always reply to the user in Italian" (nome inglese da `Language::english_name`); i messaggi evento restano in italiano |
| `session.idle_minutes` | 20 | 1..240: minuti di inattività dopo cui la conversazione interna di un segretario si chiude |
| `session.max_steps` | 8 | 1..20: passi di tool calling al massimo per risveglio |
| `secretaries` | `[]` | al massimo 20 |

Il provider è configurato (`Provider::is_configured`) quando indirizzo e modello ci sono entrambi; i due campi
si possono salvare anche uno alla volta.

**Segretario** (`HubSecretary`):

| Campo | Regole |
|---|---|
| `name` | handle della menzione: trim e minuscolo, poi 1..24 caratteri tra `a-z`, `0-9` e `-`, senza `-` iniziale o finale; unico |
| `image` | facoltativa, `{ png, color }` come le immagini dei bot (`bots::validate_image`: PNG base64, al massimo 64×64 pixel e 32 KB, colore `#rrggbb`) |
| `system_prompt` | testo libero letto dal modello (di solito in inglese), al massimo 8000 caratteri, a capo e tabulazioni ammessi. Vuoto = segretario **non configurato**: esiste ma il motore non lo fa rispondere (`Secretary::is_configured`) |
| `tools` | tool del motore che può usare (`HubSecretaryTool`): `board_status`, `thread_read`, `post_message`, `open_plan_task`, `decide_proposal`; senza ripetizioni, nell'ordine canonico. Il limite vero di cosa fa sta qui, non nel prompt. Un tool nuovo non si aggiunge da solo ai segretari esistenti: lo sceglie l'utente |
| `channels` | `{ "mode": "all" }` (Generale e ogni workspace, anche quelli futuri) oppure `{ "mode": "only", "channels": [...] }` con `general` o id di workspace esistenti (`hub/channel.rs`), da 1 a 200, senza ripetizioni |

**Nomi riservati.** I nomi dei segretari non devono mai diventare nomi delle sessioni dei terminali (`@mario` non
può essere un terminale): il frontend li legge da `hub_get_settings` e li esclude quando sceglie il nome di una
sessione (`pickBotName` in `src/lib/bots.ts`; la pool di `bots.rs` e `bots.json` non cambiano). Dettagli in
"Interfaccia", "Nomi riservati".

**Compatibilità in lettura.** `hub_settings.json` non deve finire in un backup per un valore che questa versione
non conosce (`hub/settings/tolerant.rs`): una lingua sconosciuta vale `it`, un tool sconosciuto (l'enum crescerà
con `relay_answer`, `ask_user`) si salta, una forma di `channels`
sconosciuta vale "nessun canale" (mai più permessi di quelli scelti), un segretario illeggibile si salta; il resto
del file resta. Ogni elemento saltato lascia un avviso nel log. Al salvataggio successivo i valori saltati non
vengono riscritti.

### API key

La API key del provider sta nel **portachiavi del sistema operativo** (Credential Manager su Windows, Keychain su
macOS, Secret Service su Linux), servizio `cli-companion`, conto `hub-provider-api-key`, con il crate `keyring`
(4.x, sola feature `v1`, MIT/Apache-2.0). L'accesso passa dal trait `secrets::SecretStore` (`OsKeyring` in
produzione, `MemorySecrets` nei test): un altro backend non tocca l'HUB.

- Non finisce mai nei file JSON, nel log né nei DTO: il frontend riceve solo `api_key_set: bool` (`HubSettingsView`)
  e la chiave si legge solo nel backend (`ApiKey::read`, per il client del modello). Gli errori del portachiavi
  descrivono il guasto senza citarne il contenuto.
- Validazione: trim, 1..1024 caratteri ASCII visibili (finisce in un'intestazione HTTP).
- La presenza si chiede al portachiavi la prima volta e poi resta in memoria, aggiornata da `hub_set_api_key` e
  `hub_clear_api_key`. Se il portachiavi non risponde, `api_key_set` vale `false` (errore nel log) e le
  impostazioni restano leggibili.

## Modello dati

Tipi Rust in `hub/model.rs`, esportati con ts-rs con il prefisso `Hub` (`HubThread`, `HubMessage`...).

**Canale** (`hub/channel.rs`): sul filo e su disco è una stringa, `"general"` oppure l'id di un workspace. Il nome
`general` è riservato; l'id di un workspace deve essere un nome di file sicuro (`validate::valid_workspace_id`,
al massimo 128 caratteri). In Rust è `enum Channel { General, Workspace(String) }`.

**Thread** (`HubThread`):

| Campo | Note |
|---|---|
| `id` | `thr-<millisecondi>-<n>` |
| `channel` | il canale del file che lo contiene |
| `title` | 1..120 caratteri, una riga |
| `state` | `open` \| `closed` |
| `task_ids` | task collegati (un task appartiene a un solo thread), al massimo 200 |
| `proposal_id` | proposta collegata (una proposta appartiene a un solo thread); un thread di pianificazione tiene l'ultima |
| `owner` | segretario **titolare** del thread: quello che vi ha aperto l'ultimo task di pianificazione (`open_plan_task`), `null` altrimenti. Alcuni eventi della board lo svegliano (vedi "Risvegli da evento"). Resta finché un altro segretario apre una pianificazione nel thread; non va confuso con il titolare della *sessione* ("Menzioni e risveglio"), che dura quanto la sessione. Facoltativo nei file scritti prima (`#[serde(default)]`) |
| `created_at`, `updated_at` | millisecondi Unix |
| `summary` | riassunto del thread (al massimo 10 righe, 1500 caratteri), scritto da un segretario alla chiusura della sua sessione; `null` finché nessuna sessione si è chiusa. Aggiornarlo non cambia `updated_at`. Facoltativo nei file scritti prima (`#[serde(default)]`) |
| `messages` | in ordine di arrivo |

I messaggi stanno dentro il thread: si leggono e si potano sempre insieme al loro thread, e il file del canale
resta uno solo. L'elenco (`hub_list_threads`) restituisce i riassunti, senza i messaggi.

**Messaggio** (`HubMessage`): `id` (`msg-<millisecondi>-<n>`), `author`, `kind`, `body`, `reply_to`, `event`,
`created_at`.

- `author`: `{ "type": "user" }` | `{ "type": "system" }` | `{ "type": "bot", "name": "mario" }` (un segretario:
  `name` è il suo handle). Solo i messaggi `user` svegliano i segretari.
- `kind`: `text` | `event`, ricavato da `event` (presente = `event`).
- `reply_to`: id di un messaggio **dello stesso thread**; un id che non c'è è rifiutato. Se poi il messaggio
  citato viene potato, la UI lo tratta come assente.
- `event` (`HubEvent`): dati strutturati, accanto al testo a template del messaggio, letti come dati (non come
  testo) dai segretari (`thread_read` e contesto del risveglio): `kind` (`task_created`, `task_started`, `task_done`, `task_failed`,
  `task_attention`, `task_deleted`, `proposal_created`, `proposal_accepted`, `proposal_rejected`), `workspace`
  (id del workspace della board), `task_id`, `proposal_id`, `status` (stato del task nella board), `summary`
  (riassunto del report), `verdict` (`passed` | `issues`, esito di una review), `note` (nota dell'app sul task).
  In `proposal_created` `task_id` è il task che ha fatto la proposta (il `plan` del pianificatore o la review),
  se c'è: il soggetto resta la proposta.
  Ogni evento deve nominare il suo soggetto: `task_id` per gli eventi dei task, `proposal_id` per quelli delle
  proposte. `workspace` deve essere l'id di un workspace (non `general`); `summary` e `note` al massimo 8000
  caratteri. Nuovi tipi e campi si aggiungono come varianti e campi facoltativi (compatibile in lettura).

**Riassunto** (`HubThreadSummary`): `id`, `channel`, `title`, `state`, `task_count`, `message_count`,
`waiting_user`, `owner` (il titolare del thread, `null` se non ne ha), `created_at`, `updated_at`; elenco dal più recente.

`waiting_user` (`hub/waiting.rs`) è **calcolato**, mai salvato, così non può divergere dai messaggi: per ogni
soggetto (task o proposta) conta l'ultimo evento, e il thread aspetta l'utente se almeno uno è `task_failed`,
`task_attention` o `proposal_created`. Un task fallito e poi ripartito, o una proposta decisa, non aspettano più.
Gli eventi potati dal limite dei messaggi non contano più.

## File

| File | Contenuto |
|---|---|
| `<app_data>/hub_settings.json` | `HubSettings` |
| `<app_data>/threads/<workspace_id>.json` | i thread del canale del workspace |
| `<app_data>/threads/general.json` | i thread del Generale |

Mai nella repository dell'utente. I file dei thread contengono `{ "threads": [...] }` nella busta di `store/`
(migrazioni, backup datati dei file corrotti, scrittura atomica, avvisi `store-warning`); la catena di migrazioni è
in `hub/files.rs`. I campi facoltativi hanno `#[serde(default)]`: un file scritto prima che un campo esistesse
resta leggibile. Un thread di un altro canale trovato nel file (file spostato a mano) viene escluso con un avviso
nel log.

**Caricamento pigro**: all'avvio si legge solo `hub_settings.json`. Un canale si legge dal disco la prima volta che
un comando lo chiede e resta in memoria (`HubState`, con il suo `JsonFile`) fino alla chiusura: con l'HUB spento
l'avvio non fa lavoro in più, e la protezione di `store/` contro la riscrittura di un file che non si è potuto
leggere vale per tutti i salvataggi successivi.

**Salvataggio**: ogni modifica si applica a una copia, si salva e solo poi si pubblica in memoria; se fallisce
nulla cambia; un'operazione che non tocca nessun thread non scrive (`HubState::update_when`). Un solo lock per
tutti i canali, tenuto anche durante il salvataggio: le scritture sono rare (messaggi dell'utente, eventi della
board) e non girano mai sul main thread né sotto un lock della board.

**Rimozione di un workspace**: `workspaces::remove_workspace`, dopo la board, chiama `hub::forget_workspace`, che
toglie il canale dalla memoria, butta l'istantanea della sua board e cancella `threads/<workspace_id>.json`
(nessun errore se non esiste). `board` non dipende da `hub`; `automation` solo dall'host reale (vedi "Eventi della
board"). I comandi e gli eventi della board verificano, sotto il lock dell'HUB, che il workspace del canale
esista ancora: un id inventato o un workspace appena rimosso non lasciano file orfani.

## Limiti

Assunzioni di questa tappa, tutte in costanti di `hub/text.rs` e `hub/threads.rs`:

| Limite | Valore | Comportamento |
|---|---|---|
| Testo di un messaggio | 1..8000 caratteri dopo il trim | rifiutato se vuoto o troppo lungo; ammessi a capo e tabulazioni, non gli altri caratteri di controllo |
| Titolo di un thread | 1..120 caratteri dopo il trim | rifiutato; una riga sola |
| Messaggi per thread | 500 | si tolgono i più vecchi |
| Thread chiusi per canale | 200 | alla chiusura si tolgono i chiusi aggiornati da più tempo (mai quello appena chiuso); gli aperti non si potano |
| Task collegati a un thread | 200 | rifiutato oltre |
| Nome di un bot autore | 1..24 caratteri | come i nomi della pool di `bots.rs` |

Un thread chiuso accetta ancora messaggi (per esempio gli eventi dei suoi task) e resta chiuso.

## API interna

Logica pura in `ChannelThreads` (`hub/threads.rs`, `hub/threads/lifecycle.rs`), senza orologio né generatore di
id: tempo e id li passa il chiamante. La usano i comandi e gli eventi della board (`hub/events/route.rs`).

| Operazione | Funzione |
|---|---|
| Creare un thread | `create_thread(channel, id, title, now)` |
| Aggiungere un messaggio | `add_message(thread_id, NewMessage { id, author, body, reply_to, event }, now)` |
| Chiudere / riaprire | `set_state(thread_id, ThreadState::Closed \| Open, now)` |
| Collegare task | `link_tasks(thread_id, task_ids, now)` (tutto o niente) |
| Collegare una proposta | `link_proposal(thread_id, proposal_id, now)` |
| Rendere titolare un segretario | `set_owner(thread_id, name, now)` |
| Thread di un task / di una proposta | `thread_of_task(task_id)`, `thread_of_proposal(proposal_id)` |
| Elenco | `summaries()` |

Gli errori sono in italiano (li legge l'utente); un'operazione che fallisce non cambia nulla.

## Comandi Tauri

Validano gli argomenti per primi e fanno l'I/O in `crate::blocking`. Wrapper nell'oggetto `api` di
`src/lib/tauri.ts`.

| Comando | Argomenti | Ritorna |
|---|---|---|
| `hub_get_settings` | — | `HubSettingsView` (`HubSettings` + `api_key_set`) |
| `hub_set_enabled` | `enabled` | `HubSettingsView` salvate (come `bots`: niente evento); spegnere butta le istantanee delle board |
| `hub_set_provider` | `base_url`, `model` | `HubSettingsView` |
| `hub_set_api_key` | `key` | `HubSettingsView` (la chiave va nel portachiavi) |
| `hub_clear_api_key` | — | `HubSettingsView`; nessun errore se non c'era |
| `hub_set_language` | `language` | `HubSettingsView` |
| `hub_set_session` | `idle_minutes`, `max_steps` | `HubSettingsView` |
| `hub_create_secretary` | `name`, `system_prompt`, `tools`, `channels` (`null` = tutti) | `HubSettingsView`; nuovo segretario in fondo, senza immagine |
| `hub_update_secretary` | `name`, `new_name`, `system_prompt`, `tools`, `channels` | `HubSettingsView`; sostituisce i campi, l'immagine resta; rinomina rifiutata se `new_name` è di un altro segretario |
| `hub_delete_secretary` | `name` | `HubSettingsView` |
| `hub_set_secretary_image` | `name`, `png`, `color` | `HubSettingsView` |
| `hub_remove_secretary_image` | `name` | `HubSettingsView` |
| `hub_list_threads` | `channel` | `HubThreadSummary[]`, dal più recente |
| `hub_get_thread` | `channel`, `thread_id` | `HubThread` con i messaggi |
| `hub_create_thread` | `channel`, `title` | `HubThread` (thread libero dell'utente) |
| `hub_send` | `channel`, `thread_id`, `body`, `reply_to?` | `HubMessage` dell'utente; dopo il salvataggio il motore sveglia i segretari (in background) |
| `hub_secretary_status` | — | `HubSecretaryStatus[]`: i segretari svegli adesso (vedi "Motore dei segretari") |

Con l'HUB spento `hub_create_thread` e `hub_send` sono rifiutati; i comandi delle impostazioni funzionano sempre.
Gli errori di validazione sono in italiano (li legge l'utente) e un comando che fallisce non cambia nulla; un
segretario che non c'è risponde «segretario non trovato». Un canale di un workspace che non esiste
risponde «canale non trovato: il workspace non esiste più».

## Evento

`hub-update`, emesso a ogni modifica di un thread (dai comandi, dagli eventi della board e dal motore dei
segretari: risposte, avvisi, riassunto), con payload
`HubThreadSummary` del thread toccato (riga in `TauriEvents`). Basta per aggiornare l'elenco dei thread e il
badge del canale senza rileggere nulla; la conversazione aperta rilegge il thread con `hub_get_thread` quando
`message_count` o `updated_at` cambiano.

`hub-secretary-status`, emesso dal motore dei segretari a ogni cambiamento delle sessioni, con payload
`HubSecretaryStatus[]` (l'elenco completo, vedi "Motore dei segretari"); `hub_secretary_status` lo legge all'apertura
della vista.

## Interfaccia

Interruttore in Impostazioni › Generale › Agents HUB (spento di default). L'app ha due **mondi**: i *Terminali*
(workspace, Tasks Manager, Ufficio, come prima dell'HUB) e l'*Agents HUB*. Con l'HUB **acceso** una **barra dei
mondi** (`components/rail/`, 64px, `<nav aria-label="Mondi">`) sta a sinistra, sempre visibile: Terminali, HUB (con
la pillola dei thread che aspettano l'utente), Ufficio (con i bot attivi), poi Impostazioni (col pallino di
aggiornamento) e versione. Con l'HUB **spento** non c'è barra né vista né chiamate (all'avvio si legge solo
`hub_get_settings`) e il layout è quello di prima: la sidebar ha il suo piede con Impostazioni, Ufficio e versione.

Ogni zona dello schermo resta al suo posto; cambia il contenuto con il mondo:

| Zona | Mondo Terminali | Mondo Agents HUB |
|---|---|---|
| Barra del titolo | tab dei workspace (il riquadro del logo è largo 324px con la barra, 260px senza) | percorso `Agents HUB / # canale / titolo del thread` (`HubTitlePath`) |
| Colonna di sinistra (260px) | sidebar dei workspace, senza piede se c'è la barra | canali (`HubSidebar`): Generale e uno per workspace visibile, con la pillola dei thread in attesa |
| Centro | terminali, pannello Task, Tasks Manager, Ufficio | `HubOverlay`: thread del canale e conversazione |
| Barra di stato | come sempre, più «Agents HUB: N thread aspettano te» se N > 0 | «Agents HUB», N thread in attesa, conteggio dei terminali a destra (`HubStatusBar`) |

I terminali restano sempre montati e l'area di lavoro ha la stessa dimensione nei due mondi (barra e colonna hanno la
stessa larghezza); l'overlay dell'HUB li copre come l'Ufficio, e l'HUB e l'Ufficio si escludono (`setWorld`,
`setOffice`).

**Collegamenti tra i mondi.** Terminali → HUB: il bottone con il numero di thread in attesa accanto al nome del
workspace nella sidebar apre il suo canale; «Agents HUB: N thread aspettano te» nella barra di stato apre il canale del
thread in attesa più recente (altrimenti l'ultimo canale aperto). HUB → Terminali: «Terminali» nell'intestazione dei
thread (non nel Generale) mostra i terminali di quel workspace; «Apri nel Tasks Manager» nella conversazione apre il
manager; sugli eventi della board «apri il terminale del worker» (se il task ha una `session` e il tab è ancora aperto,
`workerTabOf`) mette a fuoco il tab. Ctrl+Maiusc+H passa da un mondo all'altro (solo con l'HUB acceso e nessun dialogo
aperto); non c'è una X per chiudere l'HUB, si esce dalla barra o dalla scorciatoia.

**Persistenza.** Il mondo attivo (`UiState.world`) non si salva nella sessione, come `officeOpen` e `managerOpen`:
all'avvio si ripristinano i terminali, l'HUB è spento finché le impostazioni non sono lette e partire dai Terminali
evita un lampo di vista sbagliata. Spegnere l'HUB riporta ai Terminali.

**Caricamento.** Acceso l'HUB (e quando cambia l'insieme dei canali) `useHubSync` legge l'elenco di ogni canale, perché
le pillole servono anche nel mondo Terminali; poi `hub-update` li tiene aggiornati. Le pillole contano solo i canali
dei workspace visibili (`totalWaiting`).

| Parte | Dove |
|---|---|
| Stato (interruttore, canale, filtro, riassunti per canale, thread aperto, risposta in corso) | `stores/hub.ts` |
| Mondo attivo (`world`, esclusivo con `officeOpen`) e derivazioni (`activeWorld`, `worldRailShown`, `sidebarFooterShown`) | `stores/ui.ts`, `stores/view.ts` |
| Canali dai workspace visibili | `stores/hubChannels.ts`, `hooks/useHubChannels.ts` |
| Canali, thread in attesa, percorso del titolo, controlli sui testi, decisioni di rilettura | `lib/hub.ts` |
| Resa degli eventi per tipo (etichetta, icona, tono; i tipi sconosciuti hanno la resa generica) e tab del worker | `lib/hubEvents.ts` |
| Chiamate al backend, `hub-update` e caricamento dei canali | `app/actions/hub.ts`, `hooks/useHubSync.ts` |
| Passaggi tra i mondi | `app/actions/view.ts`, `hooks/useShortcuts.ts` |
| Impostazioni `settings` e segretari svegli `secretaryStatus` (evento `hub-secretary-status`, lettura all'accensione) | `stores/hub.ts`, `app/actions/hub.ts`, `hooks/useHubSync.ts` |
| Logica pura dei segretari: menzionabili, `@` in corso, evidenziazione, stato, titolare, "sta scrivendo" | `lib/hubSecretaries.ts` |
| Pagina Impostazioni › Agents HUB: interruttore e tre accordion | `components/settings/HubPage.tsx`, `components/settings/hub/*` |
| Controlli, riepiloghi e bozza del segretario delle impostazioni; salvataggio (ogni comando ritorna le impostazioni, che finiscono nello store) | `lib/hubSettings.ts`, `app/actions/hubSettings.ts` |
| Messaggi dei segretari (avatar, badge «SEGRETARIO»), dell'utente (bolla a destra), menzioni evidenziate | `components/hub/HubMessageItem.tsx`, `components/hub/HubUserMessage.tsx`, `components/hub/SecretaryBadge.tsx`, `components/tasks/Markdown.tsx` |
| Compositore con autocompletamento di `@` | `components/hub/HubComposer.tsx`, `MentionMenu.tsx`, `useMentionAutocomplete.ts` |
| «sta scrivendo…», titolare del thread e sessione attiva | `components/hub/TypingIndicator.tsx`, `components/hub/ThreadOwner.tsx`, `components/hub/ThreadSession.tsx`, `components/hub/ListOwner.tsx` |
| Scheda della proposta (task espandibili, stato, «Rispondi») e link al task `plan` | `components/hub/HubProposalCard.tsx`, `HubProposalTask.tsx`, `HubEventRow.tsx`, `lib/hubProposals.ts` |
| Task di pianificazione: badge «Pianificazione» e domande del pianificatore | `lib/plan.ts`, `components/tasks/PlanBadge.tsx`, `components/tasks/DetailResult.tsx` |
| Sezione SEGRETARI della colonna di sinistra | `components/hub/HubSecretaryList.tsx`, `components/hub/HubSidebar.tsx` |
| Nomi riservati ai terminali | `lib/bots.ts`, `reservedBotNames` in `stores/hub.ts`, `app/actions/tabs.ts`, `app/events.ts`, `hooks/useSessionPersistence.ts` |

Il mondo Agents HUB ha tre colonne: canali (a sinistra, nella colonna dello schermo), thread del canale (filtro
Aperti/Tutti, «+» nuovo thread) e conversazione. Gli eventi sono righe compatte con
«Rispondi»: il compositore mostra il messaggio citato (annullabile) e invia con `reply_to`; la risposta riporta il
riferimento all'originale, che sparisce se il messaggio è stato potato. Invio invia, Maiusc+Invio va a capo, al massimo
8000 caratteri.

### Impostazioni dei segretari

Impostazioni › Agents HUB (`HubPage`): l'interruttore e, sotto, tre **accordion** (`HubAccordion`: l'intestazione è un
pulsante con `aria-expanded` collegato al suo pannello `role="region"`). Ognuno mostra a destra un **riepilogo** anche
da chiuso. Le impostazioni si vedono e si salvano anche a HUB spento (`hub_get_settings` le legge all'avvio e finiscono
in `settings` di `stores/hub.ts`; ogni comando di salvataggio ritorna le impostazioni aggiornate). Finché non sono
arrivate la pagina scrive «Impostazioni non disponibili.» e gli accordion non compaiono.

| Accordion | Contenuto | Riepilogo |
|---|---|---|
| **Modello** (aperto di default) | endpoint e nome del modello (un solo modulo, «Salva» attivo solo se qualcosa è cambiato, controllo prima dell'invio); sotto, la API key | «Non configurato», «Endpoint locale» / «Endpoint remoto» / «Endpoint non valido» più «API key impostata» o «nessuna API key» |
| **Sessioni dei segretari** | minuti di inattività (1-240) e passi massimi per risveglio (1-20) | «20 min · 8 passi» |
| **Segretari** (aperto di default) | lingua delle risposte, elenco, editor, «Nuovo segretario» (disattivato oltre 20) | «3 segretari · 2 configurati · italiano» |

- **API key** (`ApiKeyField`): mostra solo se c'è (`api_key_set`), con Imposta/Cambia e Rimuovi. La chiave non si
  mostra mai e non sta in nessuno stato React né nello store: vive solo nel campo (non controllato, `type="password"`)
  finché non si invia, poi il campo si svuota, anche se il backend rifiuta. Resta nel portachiavi (vedi "API key").
  **Non c'è un pulsante «Prova la connessione»**: il backend non ha un comando per farlo; un endpoint sbagliato si
  scopre alla prima menzione, dal messaggio di sistema del segretario (vedi "Ciclo di tool calling").
- **Elenco dei segretari** (`SecretaryRow`): avatar, `@nome`, prima riga del system prompt, tool e canali ("tutti i
  canali", "Generale, sito-web" o "nessun canale"). Un segretario **non configurato** (system prompt vuoto) ha il
  bordo tratteggiato, l'avatar attenuato e la scritta «Non configurato: non risponde alle menzioni»; il pulsante è
  «Configura» invece di «Modifica». «Elimina» chiede una conferma nella riga.
- **Editor** (`SecretaryEditor`, uno solo aperto alla volta): nome (`@` fisso davanti; minuscole, cifre e `-`, fino a
  24 caratteri), immagine, system prompt (con l'avviso che lo legge il modello, quindi in inglese), tool consentiti
  (una casella per tool) e canali ("Tutti i canali, anche quelli futuri" oppure "Solo questi" con Generale e i
  workspace visibili). «Salva» invia tutto insieme; i controlli del frontend (`checkDraft`) riprendono quelli del
  backend, che resta l'ultima parola.
- **Immagine** (`SecretaryImageField`): la stessa pipeline pixel-art delle immagini dei bot (`useBotImagePicker`),
  con anteprima prima di usarla. L'immagine viaggia con comandi separati dopo il salvataggio dei campi: se non viene
  accettata il segretario è comunque salvato e l'editor lo dice («Segretario salvato, ma l'immagine no: …»).

### Conversazione

- **Messaggi** (`HubMessageItem`): un messaggio di un segretario ha l'avatar (la sua immagine, o quella
  deterministica), `@nome`, il badge **SEGRETARIO** (`SecretaryBadge`) e uno sfondo con il colore primario. Quelli
  dell'utente (`HubUserMessage`) stanno a destra, come in una chat: una bolla con sfondo `surface-2`, larga al massimo
  il 75% della conversazione, senza nome né avatar, con il riferimento della risposta e l'ora dentro la bolla.
  Segretari, sistema ed eventi restano a sinistra. Gli eventi sono righe compatte (`HubEventRow`).
- **Menzioni evidenziate**: nel testo, `@nome` di un segretario che esiste si evidenzia (`splitMentions` in
  `lib/hubSecretaries.ts`, usato da `Markdown` tramite la prop `mentions`). La grammatica è quella del backend
  (`engine/mentions.rs`): `posta@mario.it` o `@màrio` non sono menzioni; un nome che non è di un segretario resta
  testo normale. Si evidenziano anche i segretari non configurati.
- **Autocompletamento di `@`** (`HubComposer`, `MentionMenu`, `useMentionAutocomplete`): scrivendo una menzione
  compare un menu con i segretari **configurati e ammessi nel canale** (`mentionable`), il cui nome inizia con quanto
  scritto. Frecce scorrono, Invio e Tab completano (con `@nome ` e il cursore dopo), Esc chiude solo il menu. Non
  compaiono i segretari senza system prompt né quelli esclusi dal canale: come nel backend, non si sveglierebbero.
- **«<nome> sta scrivendo…»** (`TypingIndicator`): sotto i messaggi, mentre almeno un segretario ha `working` sul
  thread («mario e luisa stanno scrivendo…» se sono due). L'area `role="status"` resta nella pagina per l'annuncio
  agli screen reader.
- **Titolare del thread** (`ThreadOwner`): nell'intestazione, il segretario `owner` del thread (quello che vi ha aperto
  l'ultima pianificazione) con avatar e «· titolare». Nell'elenco dei thread ogni riga con un titolare ne mostra l'avatar
  (`ListOwner`), anche se il thread non è aperto: `owner` viaggia nel riassunto (`HubThreadSummary`) e in `hub-update`.
- **Sessione attiva** (`ThreadSession`): accanto al titolare, il segretario con la sessione che scade per ultima
  (vedi "Menzioni e risveglio"), con «sta lavorando» oppure «scade tra N min» (arrotondato per eccesso, «scade a momenti»
  a fine corsa). Niente se nessuno è sveglio; può essere un segretario diverso dal titolare.
- **Scheda della proposta** (`HubProposalCard`): l'evento `proposal_created` non è una riga ma una scheda «Proposta
  pronta · N task» con lo stato (In attesa / Accettata / Scartata, ricavato dall'ultimo evento della proposta nel thread:
  `proposalStatuses`), i task con id, modello e dipendenze, ciascuno espandibile per leggere la specifica in markdown
  (`HubProposalTask`). I task si leggono dalla board del workspace già nello store finché la proposta è pendente: l'evento
  porta solo `proposal_id`. Decisa, la proposta lascia la board e la scheda ripiega sul testo del messaggio (che elenca
  i titoli). La scheda **non decide**: «Rispondi» (in evidenza finché è pendente, con il suggerimento «Rispondi per
  accettare o scartare») cita il messaggio e la decisione passa dal segretario (`decide_proposal`).
- **Link al task**: la riga di ogni evento di un task ancora nella board ha il link «apri il task nel Tasks Manager»
  (`eventTaskOf`); per un `plan` (anche «proposta pronta» e «senza proposta») è «apri il task di pianificazione nel
  Tasks Manager». Un task eliminato o archiviato non ha link. Decisioni (`proposal_accepted` /
  `proposal_rejected`) e risvegli del titolare sono righe e messaggi del segretario come gli altri.

### Colonna SEGRETARI

Sotto i canali, `HubSidebar` mostra la sezione **Segretari** (`HubSecretaryList`): per ciascuno avatar, `@nome` e lo
stato, ricavato da `secretaryStatus` (`secretaryActivity`):

| Stato | Quando | Resa |
|---|---|---|
| **non configurato** | system prompt vuoto | avatar e nome attenuati |
| **dorme** | configurato, nessuna sessione aperta | avatar attenuato |
| **sveglio · scade tra N min** | almeno una sessione aperta; scade l'ultima | pallino verde |
| **al lavoro** | lavora su almeno un thread | pallino verde che pulsa |

Senza segretari la colonna invita a crearne uno e il collegamento «Creane uno nelle impostazioni» apre la pagina.
`secretaryStatus` si legge all'accensione (`hub_secretary_status`) e poi si sostituisce per intero a ogni
`hub-secretary-status`; con l'HUB spento l'evento si ignora e lo stato si butta. Il tempo che resta si aggiorna con
l'orologio condiviso del Tasks Manager (`useManagerNow`).

### Nomi riservati

I nomi dei segretari non diventano mai il nome di una sessione di terminale. `reservedBotNames` (`stores/hub.ts`) li
ricava dalle impostazioni e lo passano a `pickBotName` (`lib/bots.ts`) quando si apre un tab (`tabsStore.open`, da
`app/actions/tabs.ts` e `app/events.ts`) e a `assignRestoredNames` quando si ripristina la sessione
(`hooks/useSessionPersistence.ts`, che prima aspetta `loadHubSettings`): un tab ripristinato con un nome diventato
riservato ne riceve uno nuovo. Il confronto non distingue le maiuscole. **Limite**: un tab già aperto non viene
rinominato se poi nasce un segretario con quel nome; il nome cambia al prossimo avvio dell'app.

## Eventi della board

Con l'HUB acceso, ciò che succede sulla board di un workspace diventa **messaggi evento** (`author: system`,
`kind: event`) nei thread del canale di quel workspace, raggruppati per obiettivo. È la base su cui reagiranno i
bot della tappa 2. Codice in `hub/events/`.

### Aggancio

`automation::AutomationState::publish`, quando una revisione della board viene davvero pubblicata (non una
revisione vecchia scartata), dopo `emit_board` chiama `Host::board_published(view)`. `TauriHost` la inoltra a
`hub::board_published`, che con l'HUB spento non fa nulla; acceso, ricava dalla `BoardView` un'**istantanea
compatta** (`events/snapshot.rs`) e la accoda a `HubFeed` (`events/feed.rs`).

Perché una coda: `publish` tiene il lock delle pubblicazioni, così le viste arrivano nell'ordine delle revisioni.
Sotto quel lock si fanno solo l'istantanea e un invio su un canale `mpsc`; un thread dedicato
(`hub-board-events`, avviato alla prima vista con l'HUB acceso e riavviato se si ferma) riceve le istantanee
nello stesso ordine, fa il diff, instrada e salva. Il lavoro su disco dell'HUB non allunga mai il lock della
board. Il canale non è limitato: i messaggi sono istantanee compatte e arrivano al ritmo delle modifiche della
board.

### Istantanee e base di partenza

Il thread tiene in memoria l'ultima istantanea di ogni workspace: per task stato, attenzione, titolo, tipo e,
per raccontare gli eventi, riassunto del report, verdetto e nota (ripuliti dai caratteri di controllo e tagliati
a 1000 caratteri); per proposta id e task proposti.

- All'**avvio dell'app** la prima vista pubblicata di un workspace fa solo da base, senza eventi (il primo giro
  dell'automazione ripubblica tutte le board): i `running` diventati `failed` per il riavvio non generano messaggi.
- All'**accensione dell'HUB** (solo il passaggio da spento ad acceso di `hub_set_enabled`) non si pubblica nulla:
  la base di ogni board è fissata subito con la sua vista corrente, senza messaggi né scritture. Così già la
  prima modifica successiva produce i suoi eventi (per esempio la prima proposta dell'Intake crea il suo thread
  con il messaggio di proposta). Le viste arrivano da una sorgente che `lib.rs` registra
  (`automation::board_views` → `hub::set_board_source`): `hub` non dipende da `automation`. Se nel frattempo è
  già arrivata una vista pubblicata, la base resta quella.
- Ogni istantanea porta la revisione della board: una vista più vecchia della base (la vista letta
  all'accensione può essere più recente di una pubblicazione ancora in volo) viene scartata senza eventi.
- Spegnere l'HUB butta le istantanee; rimuovere un workspace butta la sua.
- Gli eventi avvenuti ad app chiusa o con l'HUB spento non vengono ricostruiti.

### Diff (`events/diff.rs`, puro)

Dall'istantanea precedente e da quella nuova, in quest'ordine:

| Evento | Quando | `kind` |
|---|---|---|
| Proposta comparsa | id nuovo tra le proposte | `proposal_created` |
| Proposta accettata | sparita e almeno uno dei suoi task è comparso nella board | `proposal_accepted` |
| Proposta scartata | sparita senza task comparsi | `proposal_rejected` |
| Task creato | comparso e non viene da una proposta accettata in questo passo | `task_created` |
| Task avviato | stato diventato `running` | `task_started` |
| Task completato | stato diventato `done` | `task_done` |
| Task fallito | stato diventato `failed` | `task_failed` |
| Attenzione | `attention` da falso a vero | `task_attention` |
| Task eliminato | sparito senza essere `done` | `task_deleted` |

Un task appena comparso parte da "in coda, senza attenzione" (un task accettato e già avviato dà anche
`task_started`). Tornare in coda (Riprova, stop, archiviazione del workspace) non è un evento: si vedrà alla
ripartenza. Un task sparito da `done` è stato archiviato e non è un evento. **Assunzione**: dalla vista non si
distingue l'archiviazione di un task `failed` (oltre i 200 task conclusi) dalla sua eliminazione: vale come
eliminato.

### Instradamento (`events/route.rs`, puro)

Sui thread del canale del workspace, con tempo e id passati dal chiamante:

- **proposta comparsa** → nuovo thread intitolato col primo task (`"<titolo> +N"` se più d'uno), `proposal_id`
  impostato, messaggio con l'elenco dei task (i primi 20, poi "… e altri N"). Fa eccezione la proposta di un
  **pianificatore** (`source_task` = un task `plan`) il cui piano è già in un thread (aperto da un segretario con
  `open_plan_task`): va in **quel** thread, che riceve il `proposal_id`. Le proposte dell'Intake e dei revisori
  continuano ad aprire un thread proprio;
- **proposta accettata** → stesso thread (creato ora se la proposta era pendente già prima dell'accensione), task
  collegati, «Proposta accettata: N task in coda.»; **scartata** → «Proposta scartata.» e thread chiuso (nessun
  thread se non ne aveva uno);
- **task creato a mano** → nuovo thread col suo titolo e quel task;
- **eventi di un task** → il thread a cui è collegato; se non ce n'è uno (task già esistente all'accensione) lo
  si crea al primo evento. Un task eliminato senza thread non ne crea uno;
- un evento di un task (non l'eliminazione) **riapre** il suo thread se era chiuso;
- un task `plan` concluso senza aver fatto una proposta (né sulla board né già riportata nel thread) scrive
  «Pianificazione conclusa senza proposta: <titolo>» con il riassunto, che contiene le domande del pianificatore;
- alla fine del passo, un thread toccato e aperto i cui task collegati sono tutti `done` o spariti dalla board si
  **chiude** con «Tutti i task sono conclusi: thread chiuso.» (messaggio di sistema senza `event`). Serve almeno un
  task che non sia un `plan` (un piano da solo aspetta la proposta o le risposte) e nessuna proposta del thread
  ancora pendente.

Nei thread con un **titolare** l'instradamento restituisce anche i risvegli (`EventWake`: thread, segretario,
messaggio da cui parte il turno), vedi "Risvegli da evento".

Testi (`events/texts.rs`), una riga ciascuno: «Task creato: <titolo>», «Task avviato: <titolo>», «Task finito:
<titolo>», «Revisione finita: <titolo> (esito: superata | problemi trovati)», «Task fallito: <titolo>» (con
` — ` e la prima riga non vuota della nota, o del riassunto se manca, tagliata a 120 caratteri con «…»), «Il task
richiede attenzione: <titolo>», «Task eliminato: <titolo>». Fa eccezione «Pianificazione conclusa senza proposta:
<titolo>», che porta sotto le domande del pianificatore (nel thread non si vedono altrove). Il riassunto del worker
non è nel testo: il campo `event` porta i dati completi (`workspace`, `task_id`/`proposal_id`, `status` e, per
`task_done`/`task_failed`, `summary`, `verdict` e `note` del tentativo concluso) e li legge il segretario. La riga
nell'interfaccia (`HubEventRow`) mostra solo il testo, più il link «apri il task nel Tasks Manager» se il task è
ancora nella board.

Un passo con eventi è **una sola** modifica atomica del canale (`HubState::update_when`, sotto il lock dell'HUB
e dopo il controllo che il workspace esista ancora), seguita da un `hub-update` per ogni thread toccato. Un passo
che non tocca nessun thread non scrive. Un evento che non si può riportare (per esempio un id non valido) finisce
nel log e non ferma gli altri; un thread creato per un collegamento che fallisce viene tolto.

## Motore dei segretari

Il motore fa parlare i segretari: quando l'utente scrive «@mario a che punto siamo?» in un thread sveglia **solo**
Mario, gli apre una sessione con un contesto piccolo, chiama il modello configurato con i soli tool consentiti a
Mario, esegue i tool in Rust e scrive la risposta nel thread con autore `{ "type": "bot", "name": "mario" }`. Gli
altri segretari restano fermi e non costano nulla. È un sottosistema del backend, staccato dai coding agents: non è
una skill di Claude Code o OpenCode e non passa da MCP. Codice in `hub/engine/`.

| File | Responsabilità |
|---|---|
| `chat.rs` | i tipi della conversazione (messaggi, chiamate a tool, richiesta, risposta, `ModelError`) e il trait `ChatModel` |
| `openai.rs` | il client reale (`OpenAiModel`, `async-openai`) e la classificazione degli errori |
| `tools.rs` | schema JSON, descrizione e validazione di ogni tool; quale board può leggere un segretario |
| `output.rs` | board e thread in JSON compatto per il modello |
| `context.rs` | system prompt, messaggio di apertura di un turno, richiesta e pulizia del riassunto |
| `mentions.rs` | le menzioni `@nome` |
| `planning.rs` | `open_plan_task` (testo del task `plan`, collegamento, titolare) e `decide_proposal` (a quale proposta si lega e la decisione), generici sulla board |
| `sessions.rs` | sessioni, titolare, code dei thread, scadenze (puro, tempo passato dal chiamante) |
| `turn.rs` | il ciclo di tool calling, la compattazione della conversazione, il riassunto |
| `texts.rs` | i messaggi di sistema in italiano |
| `runtime.rs` | lo stato Tauri `SecretaryEngine`, gli esecutori delle code e lo schedulatore delle scadenze |
| `jobs.rs` | i lavori di una coda (turno, riassunto) sull'app |
| `host.rs` | gli effetti reali: thread, board (viste e `BoardActions`), portachiavi |
| `status.rs` | `HubSecretaryStatus`, l'evento `hub-secretary-status` e il comando `hub_secretary_status` |

La logica (menzioni, sessioni, ciclo, contesto, tool) è pura o generica sui trait `ChatModel`, `TurnHost` e
`ProposalBoard`: i test usano un modello finto (`engine/fake.rs`) che risponde con sequenze scritte a mano, un host
finto e una board finta (`engine/fixtures.rs`), senza rete.

### Menzioni e risveglio

`hub_send` salva il messaggio dell'utente e poi chiama `SecretaryEngine::on_message`, che decide chi svegliare
(`Sessions::route`, puro):

- solo i messaggi dell'utente svegliano qualcuno: quelli dei bot e di sistema mai (niente catene bot-bot); gli
  eventi della board svegliano solo il titolare del thread, con le regole di "Risvegli da evento";
- una menzione è `@` a inizio testo o dopo un carattere che non può stare in un nome (`posta@mario.it` non lo è),
  seguita dal nome (lettere, cifre, `-`; il `-` finale non conta; minuscolo); un token che prosegue con altri
  caratteri di parola (`@màrio`, `@mario_1`) non è una menzione. Una menzione di un nome che non è un segretario si
  ignora;
- le menzioni di segretari **configurati** (con `system_prompt`) e **ammessi** nel canale svegliano quei
  segretari, nell'ordine del testo, ciascuno con la sua sessione (una sessione aperta sullo stesso thread si
  riprende). Un segretario non configurato o non ammesso produce un messaggio di sistema che lo spiega, in
  risposta al messaggio («@anna non è configurato: …», «@luca non può essere menzionato in questo canale.»);
- un messaggio **senza menzioni di segretari** va al **titolare** del thread: il segretario con la sessione attiva
  più recente in quel thread (che sia ancora configurato e ammesso). Senza sessioni attive non sveglia nessuno e
  resta come nota. Un messaggio che menziona solo segretari non svegliabili non passa al titolare.

### Risvegli da evento

Un secondo motivo di risveglio, oltre al messaggio dell'utente. Nei thread con un **titolare** (`owner`, il segretario
che vi ha aperto la pianificazione) questi eventi della board lo svegliano (`events/route.rs`):

| Evento | Messaggio da cui parte il turno |
|---|---|
| proposta pronta | «Nuova proposta: …» |
| pianificazione conclusa senza proposta | «Pianificazione conclusa senza proposta: …» (il riassunto porta le domande) |
| task fallito | «Task fallito: …» |
| task in attenzione | «Il task richiede attenzione: …» |
| tutti i task del thread conclusi | «Tutti i task sono conclusi: thread chiuso.» |

**Assunzione** (per contenere i costi: ogni risveglio è una chiamata al modello): avvio e completamento di un singolo
task, accettazione e scarto di una proposta (li ha decisi l'utente) e eliminazione di un task restano solo messaggi a
template. Al massimo un risveglio per thread a ogni passo della board, dall'ultimo evento che lo chiede (il contesto
porta anche i precedenti). Un thread senza titolare riceve solo i messaggi.

Dopo aver salvato i thread (e rilasciato il loro lock) il thread degli eventi chiama `SecretaryEngine::on_events`, che
accoda il turno (`Sessions::route_event`) se il titolare è ancora configurato e ammesso nel canale; altrimenti nulla
(riga nel log). Il turno parte con una **sessione nuova** che si apre dall'evento: il primo messaggio porta i dati del
thread, gli ultimi messaggi, l'evento in JSON e le indicazioni per il titolare (in inglese: raccontare in poche righe;
una proposta si presenta e si chiede all'utente di usare «Rispondi» sul suo messaggio; le domande del pianificatore si
pongono all'utente; un fallimento si spiega; la fine si riassume). Se il titolare ha già una sessione aperta sul thread
l'evento diventa il suo turno successivo, con la conversazione che ha già (due sessioni dello stesso segretario sullo
stesso thread non esistono). Un turno svegliato da un evento **non può mai** usare `decide_proposal`. Da quel momento il
titolare ha una sessione attiva: la risposta dell'utente senza menzione («sì, accetta») va a lui come sempre.

### Code dei thread

Ogni thread ha una **coda** di lavori eseguiti uno alla volta, nell'ordine di arrivo, da un task di
`tauri::async_runtime` (un esecutore per thread con lavori, che si ferma a coda vuota). Un messaggio che arriva
mentre un segretario lavora su quel thread viene accodato e trattato dopo; due menzioni nello stesso messaggio
fanno due turni in fila. Thread diversi lavorano in parallelo. Anche il riassunto di una sessione scaduta passa
dalla coda, così un risveglio successivo sullo stesso thread legge il riassunto già aggiornato. Se un esecutore
si fermasse senza svuotare la coda (un panico in un lavoro), il messaggio successivo che sveglia un segretario nel
thread ne avvia un altro.

### Sessioni

Le sessioni stanno solo in memoria (un riavvio dell'app le chiude); la memoria vera sono board, thread e riassunto.

- **Risveglio**: il contesto si costruisce da zero. System prompt = system prompt del segretario + istruzione fissa
  del motore (in inglese: chi è, che lavora con la board di coding agents, che non conosce il codice, che non
  decide questioni tecniche, che usa i tool per leggere lo stato, che parla dei workspace sempre per nome e non mostra
  mai i loro id, risposte brevi) + **dove si trova** (il Generale, o il workspace del canale col suo nome) e i
  **workspace che può consultare**, con nome e id (`workspace_names.rs`) + «Always reply to the user in
  <Language>». Il primo messaggio porta i dati del thread (titolo, canale, stato, task e proposta collegati,
  **riassunto**), gli **ultimi 10 messaggi** prima di quello che lo ha svegliato (in JSON, con i dati degli eventi)
  e il messaggio che lo ha svegliato.
- **Sessione attiva**: la conversazione interna cresce solo dentro la sessione. Un turno successivo riceve i
  messaggi comparsi nel thread dal turno precedente (tranne i suoi, al massimo 10) e il nuovo messaggio. Gli esiti
  dei tool dei turni passati si accorciano a 600 caratteri e la conversazione tiene al massimo 40 messaggi
  (togliendo i più vecchi fino a un messaggio dell'utente, così nessun esito resta senza la sua chiamata).
- **Chiusura** dopo `idle_minutes` senza attività (l'attività riparte a ogni messaggio instradato e alla fine di
  ogni turno; una sessione al lavoro o con un turno in coda non scade): il segretario chiede al modello, senza
  tool, il **riassunto aggiornato del thread** dal riassunto precedente e dalla conversazione della sessione (al
  massimo 10 righe, in inglese perché lo legge il modello); la risposta si ripulisce (niente caratteri di
  controllo né righe vuote, al massimo 10 righe e 1500 caratteri) e va in `HubThread.summary`. Se la chiamata
  fallisce, o il provider non è configurato, resta il riassunto precedente (errore nel log). Poi la conversazione
  si butta. Una sessione senza turni completati si chiude senza riassunto.
- **Timer**: un solo thread (`hub-secretary-sessions`) per tutte le sessioni, non uno per sessione: dorme fino
  alla prossima scadenza (tra 200 ms e 30 s, così un cambio di `idle_minutes` vale entro 30 secondi), chiude le
  sessioni scadute accodandone il riassunto e si ferma quando non ci sono più sessioni (riparte con la prossima).
  Il controllo è anche **pigro**: una sessione scaduta che lo schedulatore non ha ancora raccolto non fa da
  titolare, e una nuova menzione la chiude (riassunto in coda) prima di aprirne una nuova.

**Impostazioni cambiate durante una sessione.** Le sessioni aperte non si rompono: ogni turno rilegge le
impostazioni, quindi provider, modello, API key, lingua, system prompt, tool e `max_steps` valgono **dal turno
successivo** (il turno in corso finisce con quelli con cui è partito). Un segretario eliminato, rinominato,
svuotato del system prompt o escluso dal canale chiude la sua sessione al turno successivo, senza riassunto, con
«@mario non è più disponibile in questo canale: la sua sessione è chiusa.».

**Spegnere l'HUB** (`hub_set_enabled(false)`) chiude tutte le sessioni senza riassunto e annulla i lavori in corso
(`abort` degli esecutori). L'epoca del motore cresce: un lavoro già oltre l'ultimo punto di attesa non scrive più
nei thread. **Rimuovere un workspace** butta le sessioni e i lavori del suo canale.

### Ciclo di tool calling (`turn.rs`)

Un turno: conversazione → chiamata al modello con i soli tool del segretario → se il modello chiede dei tool, per
ognuno il codice controlla che sia nella lista del segretario, valida gli argomenti contro lo schema, lo esegue e
restituisce l'esito (JSON, al massimo 6000 caratteri); altrimenti il testo è la risposta del segretario, che si
scrive nel thread (ripulita, al massimo 7999 caratteri). Al massimo `max_steps` chiamate al modello per turno.

| Caso | Esito per il modello | Nel thread |
|---|---|---|
| Tool non nella lista (o inesistente) | errore «Tool `x` is not available to you…», il turno continua | «@mario ha chiesto il tool «x», che non gli è consentito: richiesta rifiutata.» |
| Argomenti non validi (JSON non valido, non un oggetto, campi sconosciuti o mancanti, valori fuori dai limiti) | errore «Invalid arguments for `x`: …», il turno continua | «@mario ha chiamato «x» con argomenti non validi: richiesta rifiutata.» |
| Errore di esecuzione (workspace inesistente, board non ammessa) | errore con il motivo, il turno continua | — |
| Ultimo passo con altre chiamate a tool | le chiamate non si eseguono e non entrano nella conversazione | «@mario si è fermato dopo N passi senza una risposta finale. …» |
| Endpoint irraggiungibile, timeout, HTTP d'errore, risposta non valida o vuota, provider non configurato, API key illeggibile | il turno finisce | «@mario non può rispondere: <motivo>.» |

Gli errori tornano al modello come `{"error": "…"}`. Nessun caso va in panico né lascia la coda bloccata: ogni
lavoro finisce e la coda passa al successivo. La conversazione resta valida anche dopo un errore (ogni chiamata a
tool ha il suo esito).

### Tool

Schema JSON e descrizione (in inglese: li legge il modello) sono scritti in `tools.rs` accanto alla struttura degli
argomenti in cui si validano (`#[serde(deny_unknown_fields)]`); un test verifica che le proprietà dello schema e i
campi della struttura coincidano. Gli argomenti vuoti valgono `{}`. Solo `open_plan_task` e `decide_proposal`
scrivono sulla board (vedi "Pianificazione dal thread").

| Tool | Argomenti | Effetto |
|---|---|---|
| `board_status` | `workspace?` (il **nome** del workspace, senza distinguere maiuscole e spazi ai bordi; vale anche l'id; al massimo 200 caratteri) | la board del workspace del canale, o di quello indicato se il segretario ha quel canale, con `workspace` (id) e `workspace_name`: conteggi per stato, task con `id`, `title`, `status`, `attention`, `waiting_on`, riassunto del report (300 caratteri) e nota (200), proposte pendenti (`BoardView::to_list_json`); al massimo 40 task, poi `omitted_tasks`. Nel Generale senza `workspace` l'errore elenca i nomi dei workspace leggibili; un nome sconosciuto elenca i nomi possibili, un nome ambiguo (due workspace con lo stesso nome) elenca gli id tra cui scegliere |
| `thread_read` | — | titolo, canale (con `workspace_name` nel canale di un workspace), stato, task e proposta collegati, riassunto e ultimi 20 messaggi (autore, ora UTC, testo tagliato a 1000 caratteri, dati dell'evento senza i campi vuoti), più `older_messages` |
| `post_message` | `body` (1..8000 caratteri) | un messaggio intermedio del segretario nel thread; la risposta finale è comunque il testo del modello |
| `open_plan_task` | `goal` (1..4000 caratteri, ripulito dai caratteri di controllo) | crea un task `plan` in coda nel workspace del canale, collegato al thread, e rende il segretario titolare; esito `{ task_id, title, status }`. Nel Generale è un errore per il modello: deve chiedere all'utente il progetto |
| `decide_proposal` | `accept` (bool), **nessun id** | accetta o scarta la proposta dell'evento a cui risponde il messaggio dell'utente; esito `{ proposal, decision, tasks_queued }` |

**Workspace per nome**: gli id restano il riferimento interno (file, eventi, comandi), ma il modello parla dei
workspace col nome. `WorkspaceNames` (`workspace_names.rs`, puro) è l'istantanea dei workspace letta dal
`WorkspaceStore` a ogni turno: un workspace rinominato si vede subito col nome nuovo, perché nessun nome è copiato
nei file dei thread. Risolve il riferimento di `board_status` (prima l'id esatto, poi il nome) e aggiunge
`workspace_name` accanto all'id nei dati degli eventi (`event.workspace_name`) e nell'intestazione del thread.
Gli id dei task restano visibili all'utente; quelli dei workspace no.

Il segretario non vede mai la API key né file del disco: i tool leggono solo board (dalla sorgente delle viste che
`lib.rs` registra con `automation::board_views`) e thread, e agiscono sulla board solo con le azioni che `lib.rs`
registra (`hub::set_board_actions`). Nessun lock dei thread o della board è tenuto durante
una chiamata al modello: i tool prendono i lock solo per leggere o scrivere, fuori dal main thread.

### Pianificazione dal thread

Il flusso della tappa 3: «@giorgio aggiungi X al progetto» → Giorgio chiama `open_plan_task` → un coding agent
pianifica → la proposta torna nello stesso thread come evento e sveglia Giorgio, che la presenta → l'utente usa
«Rispondi» sull'evento e scrive in libero («sì, accetta», «no, lascia stare») → Giorgio chiama `decide_proposal` →
gli eventi dei task svegliano Giorgio, che riassume. Codice in `engine/planning.rs` (puro), `engine/host.rs` (effetti)
e `hub/actions.rs`.

**Azioni sulla board.** `hub` non dipende da `automation`: `lib.rs` registra `hub::BoardActions` (puntatori a
funzione, come `BoardSource`) con `automation::queue_hub_plan_task` (crea il task `plan`) e
`automation::decide_hub_proposal` (lo stesso `proposal_decide` dei pulsanti Accetta/Scarta del pannello Task, con la
stessa validazione). Senza registrazione i tool rispondono al modello «The board cannot be changed right now.».

**`open_plan_task { goal }`.**

- Solo nel canale di un workspace; nel Generale l'errore chiede al modello di domandare all'utente il progetto.
- Titolo: la prima riga del goal, al massimo 72 caratteri (la board ne ammette 80), «Plan» se non resta nulla.
- Testo (inglese, lo legge il pianificatore, che non vede altro): la richiesta dell'utente **citata testualmente**
  (il messaggio del turno; in un risveglio da evento, l'ultimo messaggio dell'utente), il goal, il riassunto del thread
  e gli ultimi 10 messaggi prima del turno (autore e testo, 1000 caratteri ciascuno).
- Modello del pianificatore: la fascia `strong` dell'agente del workspace, cioè `opus` con Claude Code (vedi
  [BOARD.MD](BOARD.MD), "Task di pianificazione"): pianificare è il ragionamento più difficile e si fa una volta per
  obiettivo, mentre il core non nomina modelli. Se l'agente non dichiara le fasce vale il modello dei worker del
  workspace. Repository: la radice.
- Il task si collega al thread (`link_tasks`) e il segretario ne diventa il **titolare** (`set_owner`). Creazione e
  collegamento avvengono **sotto il lock dei thread**: il thread degli eventi della board, che aspetta lo stesso lock,
  trova il task già collegato e ne scrive gli eventi («Task creato: …») in questo thread invece di aprirne uno. Il lock
  dura la creazione (qualche comando git), mai una chiamata al modello; l'automazione non prende mai il lock dei
  thread. Un thread con già 200 task non ne crea altri.
- Le domande del pianificatore (pianificazione conclusa senza proposta) le pone il titolare svegliato dall'evento;
  quando l'utente risponde, il segretario può aprire un **nuovo** `open_plan_task` con il goal originale, le domande
  e le risposte (la descrizione del tool lo chiede; anche il testo del piano porta i messaggi recenti con le
  domande). Non c'è ancora `ask_user` (tappa 4).

**`decide_proposal { accept }`.** La decisione resta dell'utente e la garanzia è nel codice, non nel modello:

- non prende un id: il motore, all'inizio del turno, lega il turno alla proposta dell'evento a cui il messaggio
  dell'utente risponde (`planning::proposal_target`). Deve essere un turno svegliato da un **messaggio dell'utente**
  con `reply_to` su un messaggio `proposal_created` dello stesso thread e dello stesso workspace, la cui proposta è
  ancora in attesa nel thread (l'ultimo evento della proposta è `proposal_created`);
- all'esecuzione la proposta deve essere ancora pendente sulla board; poi la decide con la validazione della board;
- altrimenti (nessun `reply_to` sulla proposta, risveglio da evento, proposta già decisa) è rifiutato con un errore
  per il modello, che deve dire all'utente di usare «Rispondi» sul messaggio della proposta;
- il primo messaggio del turno lo ricorda al modello quando c'è una proposta da decidere, e la descrizione del tool
  gli chiede di domandare invece di decidere se l'intenzione non è chiara;
- il messaggio nel thread («Proposta accettata: N task in coda.» / «Proposta scartata.») lo scrivono gli eventi della
  board, come per i pulsanti; accettata, i task si collegano al thread (la proposta è sua). I pulsanti del pannello
  Task restano.

### Client del modello

`OpenAiModel` (`openai.rs`) usa `async-openai` 0.43 (MIT; feature `chat-completion` e `rustls-no-provider`) verso
`provider.base_url` + `provider.model`, con la API key letta dal portachiavi a ogni turno (`Authorization: Bearer`).
reqwest è la 0.13 già presente per l'updater, con timeout di 120 s per chiamata e 10 s per la connessione; il
provider crittografico di rustls è `ring`, come per l'updater (installato se manca), così non si compila
`aws-lc`. `OpenAIConfig` legge `OPENAI_*` dall'ambiente: ogni campo si sovrascrive, quindi nessuna variabile
dell'ambiente (per esempio una chiave OpenAI dell'utente) arriva all'endpoint scelto. Niente streaming.

Gli errori si classificano (`ModelError`) e diventano un messaggio di sistema in italiano: non raggiungibile,
tempo scaduto, HTTP 401/403 (API key), 404 (indirizzo o modello), 429, altri codici, risposta non valida o vuota.
Nel log va solo la classe dell'errore (e per gli errori di rete il messaggio di reqwest), mai il corpo della
risposta: alcuni provider ripetono parte della chiave in un 401. La feature `byot` (parametri extra di llama.cpp
come grammatica o `json_schema`) si aggiungerà quando il motore vincolerà l'output.

### Stato verso il frontend

`HubSecretaryStatus`: `channel`, `thread_id`, `secretary`, `expires_at` (millisecondi Unix: quando la sessione si
chiude se non succede nulla, con l'`idle_minutes` corrente) e `working` (sta lavorando sul thread o ha un turno in
coda: "sta scrivendo"). L'evento `hub-secretary-status` porta l'elenco completo, ordinato per canale, thread e nome,
a ogni cambiamento (risveglio, inizio e fine di un lavoro, scadenza, spegnimento); `hub_secretary_status` lo legge
all'avvio della vista. I messaggi dei segretari passano dal normale `hub-update`.
