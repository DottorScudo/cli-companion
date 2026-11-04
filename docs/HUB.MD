# Agents HUB — fondamenta

L'**Agents HUB** è una modalità opzionale di cli-companion: uno spazio stile Slack con un **canale per
workspace** più un canale **Generale**, e dentro ogni canale dei **thread** di messaggi. Questa tappa porta solo
le fondamenta nel backend: l'interruttore globale, il modello dati, la persistenza, i comandi Tauri e l'evento
verso il frontend. Non ci sono bot né modelli; gli eventi della board nei thread e la UI sono tappe successive.

Il codice sta in `src-tauri/src/hub.rs` e `src-tauri/src/hub/`. Le dipendenze e il posto del modulo nella mappa
sono in [DESIGN.MD](DESIGN.MD), "`hub/`".

## Interruttore

`hub_settings.json` nella cartella dati dell'app: `{ "enabled": false }` di default, nella busta di `store/`
(`{schema_version, data}`, prima versione con `adopt_envelope`). La struttura ha `#[serde(default)]`: i campi
della tappa 2 (provider, bot) si aggiungono senza migrazione.

Con l'HUB **spento** (default):

- non si crea nessun file (né `hub_settings.json` finché l'utente non cambia l'impostazione, né `threads/`);
- i comandi di scrittura (`hub_create_thread`, `hub_send`) rispondono «L'Agents HUB è spento: attivalo nelle
  impostazioni.»;
- le letture (`hub_list_threads`, `hub_get_thread`) funzionano e non creano file;
- board, task e il resto dell'app non cambiano. Rispegnerlo non tocca i thread su disco.

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
| `proposal_id` | proposta collegata (una proposta appartiene a un solo thread) |
| `created_at`, `updated_at` | millisecondi Unix |
| `messages` | in ordine di arrivo |

I messaggi stanno dentro il thread: si leggono e si potano sempre insieme al loro thread, e il file del canale
resta uno solo. L'elenco (`hub_list_threads`) restituisce i riassunti, senza i messaggi.

**Messaggio** (`HubMessage`): `id` (`msg-<millisecondi>-<n>`), `author`, `kind`, `body`, `reply_to`, `event`,
`created_at`.

- `author`: `{ "type": "user" }` | `{ "type": "system" }` | `{ "type": "bot", "name": "mario" }`. Il bot arriva
  nella tappa 2, ma il formato lo prevede già.
- `kind`: `text` | `event`, ricavato da `event` (presente = `event`).
- `reply_to`: id di un messaggio **dello stesso thread**; un id che non c'è è rifiutato. Se poi il messaggio
  citato viene potato, la UI lo tratta come assente.
- `event` (`HubEvent`): dati strutturati, accanto al testo a template del messaggio, letti come dati (non come
  testo) dal task degli eventi della board e dai bot della tappa 2: `kind` (`task_started`, `task_done`,
  `task_failed`, `task_attention`, `proposal_created`, `proposal_accepted`, `proposal_rejected`), `task_id`,
  `proposal_id`, `status` (stato del task nella board), `summary`. Ogni evento deve nominare il suo soggetto:
  `task_id` per gli eventi dei task, `proposal_id` per quelli delle proposte. Nuovi tipi si aggiungono come
  varianti (compatibile in lettura).

**Riassunto** (`HubThreadSummary`): `id`, `channel`, `title`, `state`, `task_count`, `message_count`,
`waiting_user`, `created_at`, `updated_at`; elenco dal più recente.

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
nulla cambia. Un solo lock per tutti i canali, tenuto anche durante il salvataggio: le scritture sono rare
(messaggi dell'utente) e i comandi girano fuori dal main thread.

**Rimozione di un workspace**: `workspaces::remove_workspace`, dopo la board, chiama `hub::forget_workspace`, che
toglie il canale dalla memoria e cancella `threads/<workspace_id>.json` (nessun errore se non esiste). `board` e
`automation` non dipendono da `hub`. I comandi verificano, sotto il lock dell'HUB, che il workspace del canale
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
id: tempo e id li passa il chiamante. La usano i comandi e la userà il task degli eventi della board.

| Operazione | Funzione |
|---|---|
| Creare un thread | `create_thread(channel, id, title, now)` |
| Aggiungere un messaggio | `add_message(thread_id, NewMessage { id, author, body, reply_to, event }, now)` |
| Chiudere / riaprire | `set_state(thread_id, ThreadState::Closed \| Open, now)` |
| Collegare task | `link_tasks(thread_id, task_ids, now)` (tutto o niente) |
| Collegare una proposta | `link_proposal(thread_id, proposal_id, now)` |
| Thread di un task / di una proposta | `thread_of_task(task_id)`, `thread_of_proposal(proposal_id)` |
| Elenco | `summaries()` |

Gli errori sono in italiano (li legge l'utente); un'operazione che fallisce non cambia nulla.

## Comandi Tauri

Validano gli argomenti per primi e fanno l'I/O in `crate::blocking`. Wrapper nell'oggetto `api` di
`src/lib/tauri.ts`.

| Comando | Argomenti | Ritorna |
|---|---|---|
| `hub_get_settings` | — | `HubSettings` |
| `hub_set_enabled` | `enabled` | `HubSettings` salvate (come `bots`: niente evento) |
| `hub_list_threads` | `channel` | `HubThreadSummary[]`, dal più recente |
| `hub_get_thread` | `channel`, `thread_id` | `HubThread` con i messaggi |
| `hub_create_thread` | `channel`, `title` | `HubThread` (thread libero dell'utente) |
| `hub_send` | `channel`, `thread_id`, `body`, `reply_to?` | `HubMessage` dell'utente |

Con l'HUB spento `hub_create_thread` e `hub_send` sono rifiutati. Un canale di un workspace che non esiste
risponde «canale non trovato: il workspace non esiste più».

## Evento

`hub-update`, emesso a ogni modifica di un thread, con payload `HubThreadSummary` del thread toccato (riga in
`TauriEvents`). Basta per aggiornare l'elenco dei thread e il badge del canale senza rileggere nulla; la
conversazione aperta rilegge il thread con `hub_get_thread` quando `message_count` o `updated_at` cambiano.

## Interfaccia

Interruttore in Impostazioni › Generale › Agents HUB (spento di default). Acceso, la sidebar mostra un pulsante accanto
a quello dell'Ufficio che apre la vista (`components/hub/`, overlay `workbench/HubOverlay.tsx` allo stesso livello
dell'Ufficio): aprire l'una chiude l'altra, e spegnere l'HUB chiude la vista. Spento, non c'è pulsante, vista né
chiamate: all'avvio si legge solo `hub_get_settings`.

| Parte | Dove |
|---|---|
| Stato (interruttore, canale, filtro, riassunti per canale, thread aperto, risposta in corso) | `stores/hub.ts` |
| Apertura della vista (`hubOpen`, esclusiva con `officeOpen`) | `stores/ui.ts`, `stores/view.ts` (`hubShown`) |
| Canali, filtro, controlli sui testi, decisioni di rilettura | `lib/hub.ts` |
| Resa degli eventi per tipo (etichetta, icona, tono; i tipi sconosciuti hanno la resa generica) | `lib/hubEvents.ts` |
| Chiamate al backend e `hub-update` | `app/actions/hub.ts`, `hooks/useHubSync.ts` |

Tre colonne: canali (Generale, poi i workspace visibili nell'ordine della sidebar, con il segnalino se un thread ha
`waiting_user`), thread del canale (filtro Aperti/Tutti, «Nuovo thread») e conversazione. All'apertura si legge
l'elenco di ogni canale; poi si segue `hub-update`, senza interrogazioni periodiche. Gli eventi sono righe compatte con
«Rispondi»: il compositore mostra il messaggio citato (annullabile) e invia con `reply_to`; la risposta riporta il
riferimento all'originale, che sparisce se il messaggio è stato potato. Invio invia, Maiusc+Invio va a capo, al massimo
8000 caratteri.
