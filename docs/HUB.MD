# Agents HUB

L'**Agents HUB** è una modalità opzionale di cli-companion: uno spazio stile Slack con un **canale per
workspace** più un canale **Generale**, e dentro ogni canale dei **thread** di messaggi. Nel backend ci sono
l'interruttore globale, il modello dati, la persistenza, i comandi Tauri, l'evento verso il frontend e gli
**eventi della board** nei thread (vedi "Eventi della board"); nel frontend la vista (vedi "Interfaccia"). Non ci
sono ancora bot né modelli.

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
- gli eventi della board non si registrano: nessuna istantanea in memoria, nessuna scrittura;
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
  testo) dai bot della tappa 2: `kind` (`task_created`, `task_started`, `task_done`, `task_failed`,
  `task_attention`, `task_deleted`, `proposal_created`, `proposal_accepted`, `proposal_rejected`), `workspace`
  (id del workspace della board), `task_id`, `proposal_id`, `status` (stato del task nella board), `summary`
  (riassunto del report), `verdict` (`passed` | `issues`, esito di una review), `note` (nota dell'app sul task).
  Ogni evento deve nominare il suo soggetto: `task_id` per gli eventi dei task, `proposal_id` per quelli delle
  proposte. `workspace` deve essere l'id di un workspace (non `general`); `summary` e `note` al massimo 8000
  caratteri. Nuovi tipi e campi si aggiungono come varianti e campi facoltativi (compatibile in lettura).

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
| Thread di un task / di una proposta | `thread_of_task(task_id)`, `thread_of_proposal(proposal_id)` |
| Elenco | `summaries()` |

Gli errori sono in italiano (li legge l'utente); un'operazione che fallisce non cambia nulla.

## Comandi Tauri

Validano gli argomenti per primi e fanno l'I/O in `crate::blocking`. Wrapper nell'oggetto `api` di
`src/lib/tauri.ts`.

| Comando | Argomenti | Ritorna |
|---|---|---|
| `hub_get_settings` | — | `HubSettings` |
| `hub_set_enabled` | `enabled` | `HubSettings` salvate (come `bots`: niente evento); spegnere butta le istantanee delle board |
| `hub_list_threads` | `channel` | `HubThreadSummary[]`, dal più recente |
| `hub_get_thread` | `channel`, `thread_id` | `HubThread` con i messaggi |
| `hub_create_thread` | `channel`, `title` | `HubThread` (thread libero dell'utente) |
| `hub_send` | `channel`, `thread_id`, `body`, `reply_to?` | `HubMessage` dell'utente |

Con l'HUB spento `hub_create_thread` e `hub_send` sono rifiutati. Un canale di un workspace che non esiste
risponde «canale non trovato: il workspace non esiste più».

## Evento

`hub-update`, emesso a ogni modifica di un thread (dai comandi e dagli eventi della board), con payload
`HubThreadSummary` del thread toccato (riga in `TauriEvents`). Basta per aggiornare l'elenco dei thread e il
badge del canale senza rileggere nulla; la conversazione aperta rilegge il thread con `hub_get_thread` quando
`message_count` o `updated_at` cambiano.

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

- La **prima** vista di un workspace dopo l'avvio dell'app o dopo l'accensione dell'HUB fa solo da base, senza
  eventi: all'avvio i `running` diventano `failed` per il riavvio e non generano messaggi.
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
  impostato, messaggio con l'elenco dei task (i primi 20, poi "… e altri N");
- **proposta accettata** → stesso thread (creato ora se la proposta era pendente già prima dell'accensione), task
  collegati, «Proposta accettata: N task in coda.»; **scartata** → «Proposta scartata.» e thread chiuso (nessun
  thread se non ne aveva uno);
- **task creato a mano** → nuovo thread col suo titolo e quel task;
- **eventi di un task** → il thread a cui è collegato; se non ce n'è uno (task già esistente all'accensione) lo
  si crea al primo evento. Un task eliminato senza thread non ne crea uno;
- un evento di un task (non l'eliminazione) **riapre** il suo thread se era chiuso;
- alla fine del passo, un thread toccato e aperto i cui task collegati sono tutti `done` o spariti dalla board si
  **chiude** con «Tutti i task sono conclusi: thread chiuso.» (messaggio di sistema senza `event`).

Testi (`events/texts.rs`): «Task creato: <titolo>», «Task avviato: <titolo>», «Task completato: <titolo>» (con il
riassunto sulla riga dopo), «Revisione completata: <titolo> (esito: superata | problemi trovati)», «Task fallito:
<titolo>» (con la nota, o il riassunto se manca), «Il task richiede attenzione: <titolo>», «Task eliminato:
<titolo>». Il campo `event` porta i dati completi: `workspace`, `task_id`/`proposal_id`, `status` e, per
`task_done`/`task_failed`, `summary`, `verdict` e `note` del tentativo concluso.

Un passo con eventi è **una sola** modifica atomica del canale (`HubState::update_when`, sotto il lock dell'HUB
e dopo il controllo che il workspace esista ancora), seguita da un `hub-update` per ogni thread toccato. Un passo
che non tocca nessun thread non scrive. Un evento che non si può riportare (per esempio un id non valido) finisce
nel log e non ferma gli altri; un thread creato per un collegamento che fallisce viene tolto.
