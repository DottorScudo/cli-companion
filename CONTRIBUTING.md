# Contribuire a cli-companion

Guida breve per chi sviluppa sul progetto: come lanciare i controlli, quale lingua usare, dove mettere il codice
e come aggiungere un comando, uno store, un contratto o un'integrazione. L'architettura è descritta in
[docs/DESIGN.MD](docs/DESIGN.MD) (mappa dei moduli, dipendenze, flussi) e il Tasks Manager in
[docs/BOARD.MD](docs/BOARD.MD).

## Preparare l'ambiente

Prerequisiti: Node >= 24 e Rust stable (`rustfmt` e `clippy` inclusi). Il progetto è Windows-first (ConPTY e WSL).

```powershell
npm ci
npm run tauri dev     # app + hot reload (frontend su http://localhost:1420)
```

Con `tauri dev` attivo, una modifica in `src-tauri/` ricompila e riavvia l'app.

## Comandi

La CI (`.github/workflows/ci.yml`) esegue gli stessi controlli, nello stesso ordine, a ogni push su `main` e a
ogni pull request, tranne la build del frontend e il cambio di versione: la build gira nella pipeline di release
(`build-windows.yml`, dentro `tauri build`). Lanciali in locale prima di aprire la pull request.

| Cosa | Comando | Note |
|---|---|---|
| Bindings TypeScript | `npm run bindings` | rigenera `src/bindings/` dai DTO Rust (ts-rs, lancia `cargo test export_bindings`); la CI fallisce se `git diff src/bindings` non è vuoto |
| Lint | `npm run lint` | ESLint (react-hooks, divieto dello stack gluestack/react-native) |
| Formato (TS) | `npm run format` / `npm run format:check` | Prettier su `src`, `tests`, `scripts` e i file di configurazione; larghezza 120; `src/bindings` escluso |
| Tipi (TS) | `npm run typecheck` | `tsc --noEmit` |
| Test (TS) | `npm test` | `node --test tests/*.test.mjs` |
| Formato (Rust) | `cargo fmt` / `cargo fmt --check` | da `src-tauri/`, larghezza 120 (`rustfmt.toml`) |
| Lint (Rust) | `cargo clippy --all-targets -- -D warnings` | da `src-tauri/`: i warning sono errori |
| Test (Rust) | `cargo test` | da `src-tauri/` |
| Build frontend | `npm run build` | `tsc && vite build` |
| Versione | `npm run bump -- 0.18.0` | unico modo per cambiarla: aggiorna `package.json`, `package-lock.json`, `src-tauri/Cargo.toml` e `Cargo.lock`; vedi [docs/RELEASE.MD](docs/RELEASE.MD) per il rilascio |

### Una nota su `WSLENV` e `cargo test`

Il backend legge il `WSLENV` ereditato (Windows Terminal lo imposta) per aggiungervi le variabili di sessione che
devono attraversare il confine verso WSL. La logica è in `pty::env::env_vars`, **pura**: riceve il valore ereditato
come argomento, quindi i test non dipendono dalla shell da cui girano e `cargo test` passa anche con `WSLENV`
impostato. Sui rami nati prima di quella separazione (con il vecchio `pty.rs` in un solo file) il test
`session_env_is_set_and_wsl_adds_wslenv` leggeva il valore reale e falliva sotto Windows Terminal: lì, e solo lì,
lancia `env -u WSLENV cargo test` (da Git Bash) o rimuovi la variabile dalla shell prima di indagare. Se scrivi un
nuovo test che dipende dall'ambiente, passa il valore come argomento invece di leggerlo.

Un'altra regola dei test di rete: **non devono mai parlare con l'app vera**. La suite gira anche dal terminale di
un worker, dove `CLI_COMPANION_SESSION`, `CLI_COMPANION_TOKEN` e `CLI_COMPANION_PORT` sono reali. Per questo le
connessioni di `ipc` passano da `connect_local`, che nei build di test va in panic se la porta è quella
predefinita o quella dichiarata in `CLI_COMPANION_PORT`: usa porte effimere e un contratto finto.

## Lingua

- **Italiano**: interfaccia, documentazione (`docs/`, README, questo file), commenti nel codice e messaggi di commit.
- **Inglese**: tutto ciò che legge il **modello**: le skill e i preset del plugin (`agents/presets/*.md`), i prompt dei
  worker e del reviewer (`board/prompts/*.md`, `board/texts.rs`, `BOOTSTRAP_PROMPT`, `STOP_BLOCK_REASON`), le istruzioni
  e le descrizioni dei tool MCP, gli errori delle RPC e le note che l'app scrive sui task. Se non sei sicuro di chi
  leggerà un testo, chiediti se finisce nel contesto di un modello: in quel caso è in inglese.
- Il testo di un task (titolo e specifica) è nella lingua della conversazione con l'Intake.
- Gli errori tipizzati (`validate::ValidationError`) portano i due testi (italiano per l'interfaccia, inglese per
  il modello) nella stessa riga della tabella: un codice con una sola lingua non compila.
- Identificatori, nomi di file e di comandi Tauri sono in inglese o nel lessico tecnico già in uso.
- Messaggi di commit: una riga di titolo con l'area (`Board UI: azioni, modificabilità e limiti letti dal backend`),
  poi, se serve, il perché.

## Dove va il codice

I moduli e le loro dipendenze sono in [docs/DESIGN.MD](docs/DESIGN.MD), "Mappa dei moduli". Le regole da tenere:

- **Logica pura separata dall'I/O.** Le decisioni (cosa avviare, quale evento ammettere, quali variabili impostare,
  quale transizione è lecita) stanno in funzioni senza `AppHandle`, processi o file, con tempo e casualità passati dal
  chiamante: `board/`, `automation::Registry`, `ipc/gate.rs`, `pty/env.rs`, `schedule.rs`, `validate/`. L'effetto
  (git, file, evento Tauri, timer) lo esegue il livello sopra, attraverso un trait (`Host`, `IpcHandler`, `Backend`) che
  i test sostituiscono con un finto. Se una funzione ha bisogno di un orologio, passalo come parametro.
- **Il core non nomina un agente.** Tutto ciò che è di un coding agent specifico sta in `agents/<nome>/`; fuori da lì si
  usano i contratti. Lo verificano `agents/architecture.rs` e `tests/architecture.test.mjs`. L'agente di un uso
  (worker, preset) si sceglie solo con `registry::resolve_agent`, che rispetta abilitazione, predefinito e contratto
  richiesto; quanto è pronto in un ambiente lo dice `agents/readiness.rs`.
- **`board` non dipende da `automation`**, e `ipc` non importa `automation`: parla con il core tramite `IpcHandler`.
- **Input validato al confine.** Branch, modelli, repository, regole, nomi e percorsi che finiscono in una riga di
  comando passano da `validate/` o dal validatore del modulo; non si interpola mai un input in una shell: si
  rifiuta (`None`/errore) ciò che non è rappresentabile in modo sicuro, non lo si "escapa".
- **Lavoro bloccante fuori dal main thread.** I comandi che lanciano processi, toccano il disco o la rete sono `async`
  e passano da `crate::blocking`; i comandi sincroni girano sul main thread e congelano la finestra.
- **Errori mai ignorati.** Rust: si propaga un `Result` con il contesto o si registra con `log::`. Frontend:
  `report("operazione fallita", err, { notify: true })` (`src/lib/errors.ts`); un errore davvero ignorabile si dichiara
  con `ignoreError("motivo")`.
- **Frontend.** Logica pura in `src/lib/` (senza React né Tauri), stato in `src/stores/`, azioni in `src/app/`, hook
  condivisi in `src/hooks/` (quelli di una feature accanto ai suoi componenti). Un componente legge da sé ciò che
  mostra con `useStore(store, selector)`; non tiene copie dello stato.
- **Un file, una responsabilità.** Quando un file mescola temi o diventa lungo, spezzalo in moduli coesi (come
  `board/`, `automation/`, `pty/`); i test lunghi vanno in `tests.rs` o `<modulo>/tests.rs`.
- **Contratto con il frontend.** Un tipo che attraversa `invoke`/`emit` è un DTO con `#[derive(TS)]` + `#[ts(export)]`;
  non si copia a mano in TypeScript.

## Ricette

### Aggiungere un comando Tauri

1. Scrivi la funzione con `#[tauri::command]` nel modulo che possiede il dominio (`pub async fn …(app: AppHandle, …)
   -> Result<T, String>`). Valida gli argomenti per primi; il lavoro bloccante va dentro `crate::blocking(move || …)`.
   La logica decisionale in una funzione pura a parte, con i suoi test.
2. Se restituisce o riceve un tipo nuovo, rendilo un DTO: `#[derive(TS)]` + `#[ts(export)]` accanto a
   `Serialize`/`Deserialize`, poi `npm run bindings` e commit dei file generati nello stesso commit.
3. Registralo nel `tauri::generate_handler![…]` di `src-tauri/src/lib.rs`. I comandi dell'app non richiedono permessi in
   `capabilities/default.json` (solo i plugin e la finestra).
4. Aggiungi il wrapper nell'oggetto `api` di `src/lib/tauri.ts`: `invoke<T>("nome_comando", { chiaveCamelCase })`.
   Tauri v2 converte gli argomenti in camelCase (`workspace_id` → `workspaceId`).
5. Se il file Rust con il comando è nuovo, aggiungilo all'elenco di `tests/tauri-args.test.mjs`: il test confronta gli
   argomenti di ogni comando registrato con le chiavi inviate da `api` e fallisce se un comando registrato non è mai
   invocato dal frontend.
6. Chi lo usa lo chiama da un'azione (`src/app/actions/`) o da un hook, non direttamente da un componente quando
   deve aggiornare uno store.

Per un **evento** verso il frontend: `emit` dal Rust (payload = DTO), la riga in `TauriEvents` (`src/lib/tauri.ts`) e un
`useTauriEvent("nome", handler)`; quelli dei coding agent e dell'automazione si registrano in
`src/hooks/useAppEvents.ts` con i gestori di `src/app/events.ts`.

### Aggiungere uno store (frontend)

1. Crea `src/stores/<tema>.ts`: l'interfaccia dello stato, lo stato iniziale, le **transizioni pure**
   `(stato, argomenti) => stato` (stesso oggetto se non cambia nulla, così non si notifica) e una factory che le lega a
   `createStore` (`src/lib/store.ts`) con metodi con nome, come `createWorkspacesStore()`; esporta l'istanza
   (`workspacesStore`). Niente React, Tauri o CSS: solo `import type` dai bindings e import con l'estensione `.ts`, così
   `node --test` lo carica senza bundler.
2. Scrivi `tests/stores-<tema>.test.mjs` sulle transizioni e sull'archivio (vedi `tests/stores-workspaces.test.mjs`).
3. Nei componenti: `useStore(store, selector, isEqual?)` (`shallowEqual` per array e oggetti piccoli). Il selettore
   derivato condiviso va in `stores/selectors.ts`; le scritture passano da funzioni di `src/app/` che leggono con
   `getState()`.

### Aggiungere una migrazione di un file su disco

I file strutturati passano da `store/`: un `JsonFile` statico con la catena di migrazioni. La versione di un file è il
numero dei suoi passi: per cambiare il formato **aggiungi un passo in coda** (una funzione pura `Value -> Value`, con il
suo test) e non modificare mai uno già rilasciato. Un file senza busta è la versione 0. Per un valore nuovo con un
default basta `#[serde(default)]`; il passo serve quando si rinomina, si sposta o si converte. Come si comporta il
recupero (backup datati, avvisi, file illeggibili) è in DESIGN.MD, "`store/`".

### Aggiungere un metodo RPC / un tool MCP

Il metodo si aggiunge a `Method` in `automation/roles.rs` (nome sul filo, `ALL`, `roles`: il `match` è esaustivo e ti
obbliga a decidere quali ruoli — `worker`, `reviewer`, `planner`, `session` — lo possono chiamare), con il gestore in
`automation/rpc.rs` (o nel modulo dell'ambito) e la definizione del tool, con descrizione in inglese, in
`mcp/tools.rs`. Un test verifica che la tabella dei ruoli e i tool esposti coincidano. Aggiorna BOARD.MD, "Tool MCP".

### Aggiungere un contratto per i coding agent

Non aggiungere un `if agent == "…"` nel core. Un file in `src-tauri/src/agents/contracts/` con il trait (e i tipi che
scambia), un campo in `Capabilities` (e in `declared()` e in `Capability`), il test in `agents/conformance.rs`,
l'implementazione nelle integrazioni esistenti (anche `None`) e nella `FakeIntegration` dei test. Poi aggiorna la guida
nella wiki. Dettagli: [docs/wiki/integrare-un-coding-agent.md](docs/wiki/integrare-un-coding-agent.md), "Se un contratto
non basta".

### Aggiungere l'integrazione di un coding agent

Una cartella `src-tauri/src/agents/<nome>/` con una struct che implementa `AgentIntegration` e i contratti che
l'agente supporta (tutti dichiarati in `capabilities()`, presenti o `None`), una riga in `INTEGRATIONS` di
`agents/registry.rs` e una nei due test architetturali (`agents/architecture.rs`, `tests/architecture.test.mjs`).
La UI e i comandi Tauri sono generici e non si toccano; `cargo test conformance` verifica da solo ogni integrazione
registrata. L'integrazione compare agli utenti solo se la abilitano (Impostazioni → Generale → Integrazioni), e ogni
funzione (preset, worker, modelli) solo se dichiara il contratto relativo. La procedura completa, la checklist e un esempio sviluppato (OpenCode) sono nella guida:
[docs/wiki/integrare-un-coding-agent.md](docs/wiki/integrare-un-coding-agent.md).

### Aggiungere un connettore

Una cartella `src-tauri/src/connectors/<nome>/` con un descrittore `static` (campi, azioni, host e capacità verso
l'HUB, `version`) e una struct che implementa `Connector` più `SourceConnector` (fonte) o `ContactConnector` (canale
di contatto); il connettore traduce servizio ↔ DTO, non consegna nell'HUB e usa solo `ctx.http()` per la rete. Una
riga in `CONNECTORS` di `connectors/registry.rs`, una in `KNOWN` di `connectors/architecture.rs` e una in
`tests/architecture.test.mjs`; tolti gli `expect(dead_code)` che clippy segnala come non più soddisfatti. I comandi
Tauri, il runtime e l'interfaccia sono generici e non si toccano; `cargo test conformance` verifica da solo ogni
connettore registrato. Procedura, errori da usare e checklist: [docs/wiki/integrare-un-connettore.md](docs/wiki/integrare-un-connettore.md);
progetto: [docs/CONNECTORS.MD](docs/CONNECTORS.MD).

### Aggiungere un preset

Una voce in `PRESETS` (`agents/presets.rs`) e un file `agents/presets/<id>.md` (SKILL.md completo, in inglese). Ogni
integrazione che dichiara `presets` lo include nel suo pacchetto e l'app espone le voci rapide `builtin:preset:<id>`
(con l'agente del workspace) e `builtin:preset:<id>@<agente>` (sottomenu “Avvia con…”). Il testo non nomina agenti né
modelli: li legge da `get_workspace_settings` (lo verifica un test in `agents/presets.rs`).

## Test

- **TypeScript**: `tests/*.test.mjs` con `node:test`. Si testano i moduli puri di `src/lib/` e `src/stores/`
  direttamente (type-stripping di Node, nessun bundler). Ci sono anche test "di contratto" che leggono i sorgenti:
  `tests/tauri-args.test.mjs`, `tests/bindings.test.mjs`, `tests/architecture.test.mjs`.
- **Rust**: test unitari in un modulo `#[cfg(test)]` in fondo al file o, se lunghi, in `tests.rs` /
  `<modulo>/tests.rs`. `automation` si prova con un `Host` finto (timer in linea), `mcp` con un `Backend` finto,
  `ipc` con porte effimere. Ogni cambiamento di comportamento porta i suoi test: percorso principale, casi limite e
  fallimenti.
- Prima di aprire la PR: `npm run bindings` (nessun diff), `npm run lint`, `npm run format:check`, `npm run typecheck`,
  `npm test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.

## Documentazione

Se cambi un flusso, un contratto o la struttura dei moduli aggiorna [docs/DESIGN.MD](docs/DESIGN.MD); se cambi il
modello, il ciclo di vita, la consegna o i tool MCP della coda, [docs/BOARD.MD](docs/BOARD.MD). I percorsi e i nomi
citati nei documenti devono esistere nel codice: cercali prima di scriverli.
