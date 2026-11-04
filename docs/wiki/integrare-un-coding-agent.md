# Integrare un nuovo coding agent

> **cli-companion non sa come funziona un coding agent. Offre dei contratti;
> l'integrazione implementa quelli che le servono, ed è il contratto
> implementato a far funzionare l'agente.**

Questa guida spiega come aggiungere un agente (Claude Code è l'integrazione di
riferimento, OpenCode l'esempio sviluppato in fondo). Vale una regola sola:
**il core non nomina mai un agente specifico**. Tutto ciò che è proprio di un
agente (comandi, flag, formato degli hook, cartelle di configurazione, sintassi
dei permessi) vive in `src-tauri/src/agents/<id>/`. Un test architetturale
(`src-tauri/src/agents/architecture.rs` e `tests/architecture.test.mjs`) fallisce se fuori da quella cartella compare un
riferimento a un agente.

Indice: [Il modello](#il-modello) · [I contratti](#i-contratti) ·
[Procedura](#procedura-passo-passo) · [Checklist](#checklist-finale) ·
[Esempio: OpenCode](#esempio-opencode)

## Il modello

Un'integrazione è una struct che implementa il trait `AgentIntegration`
(`agents/contracts/integration.rs`).

**Nucleo obbligatorio** (senza non esiste l'agente):

| Metodo | Scopo |
| --- | --- |
| `id()` | Id stabile, `[a-z0-9-]` senza trattini ai bordi. Compare in `--notify`, nelle cartelle del pacchetto, nell'API e nel campo `agent` dei task. Non si cambia più. |
| `display_name()` | Nome mostrato nell'interfaccia. |
| `detect(&Target)` | Rileva l'agente in un target (Windows o una distro WSL) e ritorna `Detection { found, version, path }`. Può lanciare processi: il core lo chiama fuori dal main thread, con timeout (`detection::DETECT_TIMEOUT`). |
| `interactive_command()` | Riga digitata nella shell del tab per avviare l'agente. Deve valere in PowerShell, cmd e bash (niente `$`, backtick, `%`, `!`). È anche la base dell'avvio integrato `builtin:<id>`. |

**Capabilities** (tutto il resto): `capabilities()` ritorna una struct
`Capabilities` con un campo `Option<&dyn Trait>` per ogni contratto
(`agents/contracts/capabilities.rs`). `Capabilities` **non ha `Default`**: ogni
integrazione elenca tutti i campi, `Some(self)` o `None`. Quando si aggiunge un
contratto al core, ogni integrazione smette di compilare finché non decide, ed è
voluto: non esistono implementazioni di default "non supportato" che nascondano
una lacuna. Il test di conformità controlla che `declared()` coincida con i campi
presenti.

Regola per chi integra: **dichiara solo ciò che sai fare davvero**. Un contratto
`None` è una scelta legittima e il core la gestisce (vedi la tabella di ogni
contratto); un contratto dichiarato ma fatto male è un difetto.

| Contratto | Campo | Trait | File del contratto |
| --- | --- | --- | --- |
| Pacchetto | `package` | `IntegrationPackage` | `contracts/package.rs` |
| Eventi degli hook | `hook_events` | `HookEvents` | `contracts/hook_events.rs` |
| Ripresa della sessione | `session_resume` | `SessionResume` | `contracts/session_resume.rs` |
| Automazione dei worker | `automation` | `WorkerAutomation` | `contracts/automation.rs` |
| Permessi dei worker | `worker_permissions` | `WorkerPermissions` | `contracts/worker_permissions.rs` |
| Preset | `presets` | `PresetLauncher` | `contracts/presets.rs` |
| Skill utente | `user_skills` | `UserSkills` | `contracts/user_skills.rs` |
| Limiti di utilizzo | `usage_limits` | `UsageLimits` | `contracts/usage_limits.rs` |
| Consumi della sessione | `session_usage` | `SessionUsageReader` | `contracts/session_usage.rs` |
| Profilo di input | `input_profile` | `InputProfileSource` | `contracts/input_profile.rs` |

Il frontend non conosce i contratti per nome di agente: riceve i dati
dichiarativi (`agent_profiles`: contratti dichiarati, regole di input, dati di
ripresa, avviso sui limiti, testi dei permessi) e agisce su quelli.

### I due canali tra agente e app

Due meccanismi dell'app, indipendenti dall'agente, sono ciò che l'integrazione
deve collegare all'agente:

- **Canale degli hook** (`src-tauri/src/ipc/`). L'agente esegue
  `<exe> --notify <evento> <agent_id>` ogni volta che succede qualcosa. Eventi
  ammessi: `session-start`, `finished`, `session-end`, `needs-input`,
  `permission-request` (`HookEventKind`). L'exe legge le variabili
  `CLI_COMPANION_SESSION` (tab di appartenenza; senza, l'evento è ignorato con
  exit 0), `CLI_COMPANION_TOKEN` (il segreto di quella sessione, non un token globale dell'app) e `CLI_COMPANION_PORT`, che l'app imposta nel
  terminale (su WSL anche in `WSLENV`). Lo stdin dell'hook, se presente, passa da
  `HookEvents::normalize`. L'unico output ammesso su stdout è la decisione di
  blocco del fine turno. Dettagli in `docs/DESIGN.MD`, sezione `ipc/`.
- **Server MCP** (`src-tauri/src/mcp/`). L'agente avvia `<exe> --mcp` come server
  MCP stdio chiamato `cli-companion`. Eredita le stesse variabili d'ambiente del
  terminale: fuori da cli-companion espone zero tool. I tool (`get_task`,
  `complete_task`, `propose_tasks`, ...) dipendono dal ruolo della sessione
  (worker, revisore, sessione normale): vedi `docs/BOARD.MD`.

Un agente che non sa eseguire un comando a un evento, o non sa caricare un server
MCP, non può dichiarare `package` in modo utile: la maggior parte dei contratti
sotto dipende dal pacchetto.

## I contratti

Per ognuno: scopo, firma, cosa fa il core quando c'è e quando manca, quali test
di conformità (`agents/conformance.rs`) lo riguardano. I test girano su ogni
integrazione registrata e su una `FakeIntegration` (`agents/testing.rs`), e la
suite stessa è provata contro integrazioni volutamente rotte (`Flaw`).

### 1. Pacchetto: `IntegrationPackage`

**Scopo.** I file che l'app genera e installa *dentro* l'agente perché
l'agente parli con l'app: gli hook che chiamano `--notify`, la dichiarazione del
server MCP `cli-companion` (`<exe> --mcp`) e le skill dei preset.

**Firma** (`contracts/package.rs`).

```rust
fn version(&self, ctx: &InstallCtx, target: &Target) -> Option<String>;
fn files(&self, ctx: &InstallCtx, target: &Target) -> Result<Vec<PackageFile>, String>;
fn is_installed(&self, target: &Target, det: &Detection) -> bool;
fn install(&self, ctx: &InstallCtx, target: &Target, det: &Detection) -> Result<String, String>;
fn uninstall(&self, ctx: &InstallCtx, target: &Target, det: &Detection) -> Result<String, String>;
```

- `InstallCtx { exe, data_dir }` porta l'exe corrente (il comando degli hook) e la
  cartella dati; `exe_for(target)` lo traduce per WSL (`/mnt/c/...`),
  `plugin_root(agent_id, target)` dà la cartella del pacchetto generato:
  `<dati app>/agent-plugins/<id>/<target>/`.
- `version` e `files` sono **pure**: nessun processo, nessun disco. `version`
  deve cambiare quando cambiano gli hook, la dichiarazione MCP o le skill (per
  Claude Code è `<release>-h<hash>`): il core la confronta con quella installata
  per segnalare i pacchetti da aggiornare. `None` = non calcolabile, mai da
  aggiornare.
- `files` ritorna path relativi, con slash dritti, senza `..`, senza duplicati e
  senza BOM.
- `install`/`uninstall` ritornano il log dei passi. Il core li serializza per
  coppia (agente, target) con `with_target_lock`.

**Core con il contratto.** Interfaccia di installazione/rimozione/aggiornamento
(comandi `install_agent_plugin`, `remove_agent_plugin`, `agent_plugins_update`),
stato "installato" per ogni ambiente, avviso "da aggiornare".

**Core senza il contratto.** L'agente compare nell'elenco, ma `installed` è sempre
`false` e l'installazione risponde "non ha un pacchetto da installare". Senza
pacchetto non arrivano hook né MCP: niente notifiche di fine lavoro, niente
worker, niente preset.

**Conformità** (`package`): `version` non vuota, senza spazi e stabile su due
target; `files` riuscito per Windows e WSL; path relativi, con slash dritti, senza
`..`, senza duplicati, senza BOM.

### 2. Eventi degli hook: `HookEvents`

**Scopo.** Normalizzare lo stdin grezzo che l'agente passa all'hook in un
`HookEvent` neutro, e dire come si fa proseguire un turno che l'app ha bloccato.

**Firma** (`contracts/hook_events.rs`).

```rust
fn normalize(&self, kind: HookEventKind, raw: &str) -> HookEvent;
fn stop_block_output(&self, reason: &str) -> String;
```

- `HookEvent` ha solo campi opzionali: `agent_session_id`, `notification_type`,
  `message`, `already_continuing`, `background_work`, `start_source`, `end_reason`,
  `transcript_path`, `tool_name`, `tool_summary`. Si riempie ciò che l'agente
  fornisce. I nomi dei campi dell'agente (`session_id`, ...) restano nel file
  dell'integrazione.
- `normalize` è tollerante: input vuoto o malformato dà un evento senza dettagli,
  mai un errore.
- `stop_block_output` è la riga scritta su stdout dall'hook di fine turno quando
  l'app risponde `{"block": "<motivo>"}` (un worker che chiude il turno senza
  aver chiamato `complete_task`): deve far proseguire l'agente. È l'unico
  output che `--notify` produce.

**Core con il contratto.** `--notify` legge lo stdin (tetto 1 MB) e lo passa a
`normalize`; l'app usa i dettagli per l'id della sessione dell'agente (ripresa),
il transcript (consumi), il testo dei permessi richiesti e per distinguere un
fine turno vero da uno bloccato o con lavoro in background.

**Core senza il contratto.** L'evento parte lo stesso, senza dettagli, e non c'è
mai output di blocco: le notifiche funzionano, ma niente id di sessione (la
ripresa non ha cosa riprendere), niente consumi e un worker che chiude il turno
senza consegnare non viene fatto proseguire.

**Conformità** (`hook_events`): per ogni `HookEventKind` gli input `""`,
`"not json"`, `"[]"`, `"{}"` danno `HookEvent::default()`; `stop_block_output("x")`
non è vuoto e contiene il motivo.

### 3. Ripresa della sessione: `SessionResume`

**Scopo.** Dire al frontend come riprendere la conversazione di un tab dopo il
riavvio dell'app. È solo dati: il frontend non sa nulla del modo in cui un agente
riprende una sessione.

**Firma** (`contracts/session_resume.rs`).

```rust
fn resume_spec(&self) -> ResumeSpec;
// ResumeSpec { command_names, strip_flags: Vec<ResumeFlag>, resume_template }
```

- `command_names`: nomi del programma (minuscolo, senza spazi); il primo è il
  canonico, usato quando il tab non sta eseguendo l'agente. Servono al frontend
  per riconoscere di quale agente è un tab dal comando di avvio.
- `strip_flags`: flag di ripresa/continuazione da togliere dal comando originale
  (`names` con il trattino, `takes_value`).
- `resume_template`: modello con i segnaposto `{command}` e `{id}`, ciascuno
  **una volta**.

L'id che sostituisce `{id}` è quello riportato dall'hook `session-start`
(`HookEvent::agent_session_id`): senza `hook_events` non c'è.

**Core con il contratto.** Il frontend salva l'id di sessione dell'agente e alla
riapertura rilancia il comando di ripresa.

**Core senza il contratto.** Il tab si riapre con il comando originale (o una
shell vuota): nessuna ripresa.

**Conformità** (`session_resume`): `command_names` non vuoto e minuscolo; il
programma del comando interattivo è tra i `command_names`; nomi dei flag che
iniziano con `-`; ogni segnaposto esattamente una volta; il comando risultante è
sicuro per la shell.

### 4. Automazione dei worker: `WorkerAutomation`

**Scopo.** Costruire il comando che avvia una sessione automatica (un worker del
Tasks Manager) senza utente.

**Firma** (`contracts/automation.rs`).

```rust
fn command(&self, env_kind: TargetKind, req: &AutomationRequest) -> Option<String>;
// AutomationRequest { bootstrap_prompt, permission_mode, model, permissions_file }
```

- `bootstrap_prompt`: ASCII sicuro, garantito dal chiamante; chiede all'agente di
  chiamare `get_task`.
- `permission_mode`: `Plan` | `Default` | `AcceptEdits` | `Bypass`
  (`PermissionMode::ALL`, dal più restrittivo). L'integrazione mappa ciascun
  livello su ciò che l'agente offre.
- `model`: già validato (`[A-Za-z0-9._-]{1,64}`), ma l'integrazione **rivalida**.
- `permissions_file`: file di regole preparato dal contratto `WorkerPermissions`
  *della stessa integrazione*, già nel formato di path del target.
- Il risultato è digitato nella shell del tab: valido in PowerShell, cmd e bash.
  `None` = richiesta non rappresentabile in modo sicuro. Tutto ciò che non
  si può mettere letterale tra doppi apici si rifiuta (`is_shell_safe_path` per i
  path).
- Il comando deve far partire l'agente **con il server MCP `cli-companion`
  utilizzabile senza conferme** (per Claude Code una regola di permesso sul
  server): un worker che non può chiamare `get_task` non fa nulla.

**Core con il contratto.** `automation_command(agent_id, env_kind, req)` è usato
dallo scheduler per avviare i worker; l'agente può comparire come scelta per un
task.

**Core senza il contratto.** `"<nome> does not support automated sessions"`: i
task non partono con quell'agente. `registry::default_worker_integration()` (la
prima del registro) **deve** dichiarare `automation`: lo verifica la conformità.

**Conformità** (`automation`): per ogni target e ogni `PermissionMode` esiste un
comando che contiene il prompt ed è sicuro per la shell; prompt con `" $ \` % ! \\`
o a capo, o vuoto, rifiutati; modello valido accettato, modelli non validi
(vuoto, spazi, `;`, `"`, 65 caratteri) rifiutati; `permissions_file` con
caratteri pericolosi rifiutato, uno sicuro usato nel comando.

### 5. Permessi dei worker: `WorkerPermissions`

**Scopo.** Le regole allow/deny del workspace, nel formato che l'agente sa
leggere. Il core non conosce la sintassi delle regole: le valida, le scrive e le
descrive solo tramite questo contratto.

**Firma** (`contracts/worker_permissions.rs`).

```rust
fn rule_format(&self) -> RuleFormat;        // max_rules, max_rule_chars, example (inglese)
fn guide(&self) -> PermissionsGuide;        // testi dell'interfaccia (italiano)
fn validate_rules(&self, rules: &[String]) -> Result<Vec<String>, ValidationError>;
fn prepare(&self, data_dir: &Path, workspace_id: &str, env_kind: TargetKind,
           allow: &[String], deny: &[String]) -> Result<Option<String>, String>;
```

- `validate_rules`: trim, righe vuote e doppioni scartati, `Err` tipizzato
  (`ValidationError`) per l'interfaccia e per il modello.
- `prepare`: scrive (solo se cambia) un file nella cartella dati e ritorna il path
  per `AutomationRequest::permissions_file`; `None` se non ci sono regole; `Err`
  se il risultato non è usabile in modo sicuro in una riga di comando.
- `guide`: sintassi, avvertenza del bypass, segnaposto dei campi. Sono testi
  per l'utente, quindi in italiano; `RuleFormat::example` lo legge il modello
  (schema MCP): inglese.

**Core con il contratto.** `validate_worker_rules`, `worker_rule_format`,
`worker_permissions_file` (modulo `worker_rules.rs`) lavorano con l'integrazione
predefinita dei worker e con quella indicata dal task; la pagina Worker delle
impostazioni mostra la guida.

**Core senza il contratto.** Una lista vuota resta vuota; una lista con regole è
rifiutata con `ErrorCode::PermissionRulesUnsupported` (non verrebbero mai
applicate); `prepare` dà `None`; la guida è `null` e l'interfaccia non mostra i
campi delle regole.

**Conformità** (`worker_permissions`): forma delle regole completa; guida non
vuota; righe vuote scartate; l'esempio è una regola valida e invariata da
`validate_rules`; senza regole `prepare` dà `Ok(None)`.

### 6. Preset: `PresetLauncher`

**Scopo.** Sessioni interattive guidate da una skill (Task Intake, Skill Wizard,
Project Setup). Il catalogo è del core (`agents/presets.rs`, `PRESETS`, testi in
`agents/presets/<id>.md`, **in inglese**: li legge il modello).

**Firma** (`contracts/presets.rs`).

```rust
fn command(&self, preset: &Preset) -> String;     // avvia l'agente con la skill come primo prompt
fn skill_path(&self, preset: &Preset) -> String;  // dove il pacchetto spedisce la skill
```

**Chi dichiara `presets` deve spedire nel proprio pacchetto la skill di *ogni*
preset del catalogo**, al path `skill_path`, con contenuto identico a
`preset.skill_text()`. L'integrazione adatta solo il *contenitore* (cartella,
nome file, frontmatter che l'agente richiede), mai il testo del preset, e deve
poter invocare la skill dal primo prompt. Richiede quindi `package`.

**Core con il contratto.** `launches.rs` aggiunge un avvio `builtin:preset:<id>`
per ogni preset, usando l'agente predefinito se li dichiara, altrimenti il primo
che può. Il comando di ripristino del tab è quello interattivo (riprende la
conversazione invece di rieseguire la skill).

**Core senza il contratto.** Nessun avvio di preset da quell'agente; se nessun
agente li dichiara non ce ne sono affatto.

**Conformità** (`presets`): richiede `package`; ogni comando è sicuro per la
shell; `skill_path` univoci; la skill di ogni preset è nel pacchetto e coincide col
catalogo.

### 7. Skill utente: `UserSkills`

**Scopo.** Dire dove l'agente legge le skill personali. L'app archivia le skill
dell'utente (`<dati app>/user-skills`, gestite dal preset Skill Wizard e dai tool
MCP `save_skill`/`list_skills`/`delete_skill`) e le sincronizza in queste cartelle.

**Firma** (`contracts/user_skills.rs`).

```rust
fn windows_dir(&self) -> Result<PathBuf, String>;       // cartella su Windows
fn wsl_home_subpath(&self) -> &'static str;             // relativa alla home Linux, slash dritti
```

**Core con il contratto.** `user_skills/targets.rs` aggiunge un target per
l'agente su Windows e in ogni distro in uso.

**Core senza il contratto.** Le skill non vengono copiate per quell'agente.

**Conformità** (`user_skills`): `wsl_home_subpath` non vuoto, relativo, senza
backslash né `..`.

### 8. Limiti di utilizzo: `UsageLimits`

**Scopo.** Le finestre di limite dell'abbonamento (5 ore, settimanale).

**Firma** (`contracts/usage_limits.rs`).

```rust
fn limits(&self, target: &Target) -> UsageLimitsReply;  // bloccante (rete)
fn notice(&self) -> &'static str;                       // avviso di affidabilità (italiano)
```

`UsageLimitsReply { state, five_hour, seven_day, message }` con `state` in
`ok | expired | unavailable | disabled | unsupported`.

**Core con il contratto.** Il comando `agent_usage_limits` interroga l'agente;
l'avviso `notice` compare accanto all'interruttore dei consumi.

**Core senza il contratto.** Stato `unsupported`, nessun avviso: l'interfaccia
non mostra i limiti di quell'agente. È la scelta giusta per un agente senza
abbonamento a finestre o con limiti non leggibili in modo affidabile.

**Conformità** (`usage_limits`): `notice` non vuoto.

### 9. Consumi della sessione: `SessionUsageReader`

**Scopo.** Token di una sessione letti dal transcript.

**Firma** (`contracts/session_usage.rs`).

```rust
fn session_usage(&self, target: &Target, transcript_path: &str) -> Result<SessionUsage, String>;
```

`transcript_path` arriva dall'evento hook (`HookEvent::transcript_path`) come
path visto dall'agente nel target. L'input è non fidato: va validato (vuoto e
`..` rifiutati). Bloccante (disco): il core lo chiama fuori dal main thread. Una
cache è interna all'integrazione.

**Core con il contratto.** Il comando dei consumi mostra token, contesto e modello.

**Core senza il contratto.** `Ok(None)`: nessun dato, nessun errore.

**Conformità** (`session_usage`): i path `""` e `"../x"` sono rifiutati.

### 10. Profilo di input: `InputProfileSource`

**Scopo.** Tasti propri dell'agente nel terminale, in forma dichiarativa. Oggi
l'unico evento è `ImagePaste` (incolla con appunti senza testo): al posto del
Ctrl+V il terminale invia la sequenza della regola.

**Firma** (`contracts/input_profile.rs`).

```rust
fn input_profile(&self) -> InputProfile;   // InputProfile { rules: Vec<InputRule> }
// InputRule { event: InputEvent, env_kind: Option<TargetKind>, send: String }
```

**Core con il contratto.** Il frontend lo legge all'avvio e lo risolve con
`resolveInputOverride`.

**Core senza il contratto.** Profilo vuoto: il terminale mantiene il comportamento
standard.

**Conformità** (`input_profile`): `send` non vuoto; nessuna regola duplicata per
(evento, ambiente).

## Procedura passo passo

1. **Leggi l'integrazione di riferimento.** `agents/claude_code/` ha un file per
   contratto (`package.rs`, `hook_events.rs`, `resume.rs`, `automation.rs`,
   `permissions.rs`, `presets.rs`, `skills.rs`, `usage_limits.rs`,
   `session_usage.rs`, `input_profile.rs`) più il rilevamento in `mod.rs`.
   Copia la *struttura*, non i dettagli.
2. **Studia l'agente.** Per ogni contratto che vuoi dichiarare, scrivi su carta come
   l'agente lo realizza e *dove lo hai letto* (documentazione ufficiale, non
   memoria). Ciò che non è documentato si verifica a mano prima di scriverlo nel
   codice (vedi il punto 8).
3. **Crea la cartella `src-tauri/src/agents/<nome>/`** con `mod.rs`: una struct
   unit (`pub struct MioAgente;`), le costanti (`AGENT_ID`, comando interattivo,
   nomi propri dell'agente) e `impl AgentIntegration`. `capabilities()` elenca
   **tutti** i campi: `Some(self)` per i contratti implementati, `None` per gli
   altri. Un file per contratto, con i test del file in fondo.
4. **Rilevamento.** `detect` usa `target.run_timeout(.., DETECT_TIMEOUT)` e
   `parse_version`; su WSL risolvi il path assoluto con
   `resolve_wsl_command_cached` (scarta i programmi Windows visibili dal `PATH`
   di WSL). Il path assoluto serve a tutti i comandi successivi.
5. **Pacchetto.** Genera i file (hook → `--notify <evento> <id>`, server MCP
   `cli-companion` → `<exe> --mcp`, skill dei preset) in modo puro; calcola la
   versione da tutto ciò che genera; implementa installazione, stato e rimozione
   con i comandi o i file di configurazione dell'agente. Idempotente: reinstallare
   non deve lasciare residui (vedi `remove_obsolete_skills` in Claude Code).
   Scrivere in configurazioni dell'utente richiede un marcatore per riconoscere
   ciò che hai scritto tu, e la rimozione deve toccare solo quello.
6. **Eventi, ripresa, automazione, permessi, preset, ...** nell'ordine che ti
   serve. Tutto ciò che finisce in una riga di comando è non fidato: valida e
   rifiuta (`None`) invece di fare escape.
7. **Registra l'integrazione**: `mod <nome>;` in `agents/mod.rs` e una riga in
   `INTEGRATIONS` di `agents/registry.rs`. Il registro è l'unico punto in cui il
   core nomina gli agenti. L'ordine conta: la prima è l'agente predefinito dei
   worker. Aggiungila *dopo* quella esistente, a meno che non debba diventare il
   default. Poi dichiara l'agente ai test architetturali: una riga in
   `INTEGRATIONS` di `agents/architecture.rs` (cartella, id, comandi) e una in
   quello di `tests/architecture.test.mjs`. Senza la riga Rust il test fallisce
   ("le cartelle di agents/ e INTEGRATIONS devono coincidere").
8. **Esegui i test di conformità.** `cargo test conformance` verifica da solo ogni
   contratto dichiarato. Non c'è niente da registrare nella suite. Poi aggiungi i
   test del file di ogni contratto (casi principali, bordi, fallimenti).
9. **Verifica manuale** su un'installazione reale (dev: `npm run tauri dev`):
   - Impostazioni → Coding agent: la riga compare, rileva versione e percorso su
     Windows e in una distro WSL; "Installa" e "Rimuovi" funzionano; "Prova
     notifica" riesce.
   - Avvio integrato `builtin:<id>`: l'agente parte nel tab; a fine turno
     arrivano suono e ✓; una richiesta di permesso dà ❗.
   - Riavvio dell'app: il tab riprende la conversazione (se `session_resume`).
   - Un worker: un task in coda parte con il comando di automazione, l'agente
     chiama `get_task`, consegna con `complete_task` (se `automation`).
   - Permessi: regole allow/deny rispettate in un worker (se
     `worker_permissions`).
   - Preset: ognuno dei tre si avvia e carica la skill.
   - Ogni voce "da verificare" della tua analisi del punto 2.
10. **Documenta.** Aggiorna `README.md` solo se serve all'utente (installazione
    dell'agente) e `docs/DESIGN.MD`, sezione `agents/`, con una sottosezione per la
    tua integrazione. Questa guida cambia solo se cambia un contratto.
11. **Il test architetturale deve restare verde** (`cargo test architecture`,
    `npm test`): se fallisce hai scritto il nome dell'agente fuori dalla sua
    cartella. Il messaggio indica file e riga. Il test legge il solo codice di
    produzione: esclusi commenti, `#[cfg(test)]`/`#[test]` e i moduli dichiarati
    `#[cfg(test)] mod x;` (Rust); commenti e file `*.test.*`, `__tests__`,
    `fixtures` (frontend). Cerca il modulo dell'agente (solo `mod.rs` e
    `registry.rs` di `agents/` possono dichiararlo), le stringhe che contengono il
    suo id e le stringhe uguali ai suoi comandi. Sposta il dettaglio dentro un
    contratto (o, se il contratto non basta, estendilo: vedi sotto).

### Se un contratto non basta

Non aggiungere un `if agent == "..."` nel core. Estendi il contratto (o
aggiungine uno): un file in `contracts/`, un campo in `Capabilities` (e in
`declared()` e `Capability`), il test in `conformance.rs`, l'implementazione nelle
integrazioni esistenti (anche `None`) e la `FakeIntegration`. Poi aggiorna questa
guida.

## Checklist finale

- [ ] `agents/<nome>/` ha un file per contratto, tutti i campi di `Capabilities`
      elencati, nessun `Default`.
- [ ] `id` stabile, `[a-z0-9-]`; registrato in `registry.rs` (una riga).
- [ ] Il nome dell'agente non compare fuori dalla sua cartella
      (test architetturale verde, Rust e frontend).
- [ ] `detect` funziona su Windows e WSL, con timeout, e non scambia un programma
      Windows per uno Linux.
- [ ] Pacchetto: `files` e `version` puri e stabili; install/uninstall idempotenti
      e senza residui; hook su `session-start`, `finished`, `needs-input`
      (e `permission-request`/`session-end` se l'agente li ha); server MCP
      `cli-companion` con `--mcp`.
- [ ] Se dichiari `presets`: la skill di *tutti* i preset di `PRESETS` è nel
      pacchetto, testo identico al catalogo, invocabile come primo prompt.
- [ ] Se dichiari `automation`: ogni `PermissionMode` mappato; il server MCP è
      utilizzabile senza conferme; prompt, modello e path non fidati rifiutati.
- [ ] Se dichiari `worker_permissions`: l'esempio di `RuleFormat` è valido;
      `prepare` scrive solo se cambia.
- [ ] Testi per l'utente in italiano, testi letti dal modello (skill, preset,
      esempi di regole, errori RPC) in inglese.
- [ ] Nessuna stringa dell'utente interpolata in una shell: prompt, modello e
      path validati, input degli hook parsato con tolleranza.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
      `env -u WSLENV cargo test` (con `WSLENV` impostato un test di `pty` fallisce
      a prescindere), `npm run lint`, `npm run typecheck`, `npm test`.
- [ ] Verifica manuale della procedura (punto 9) fatta su un'installazione reale.
- [ ] `docs/DESIGN.MD` aggiornato.

---

## Esempio: OpenCode

Questo è un *esercizio di progetto*, non codice: come [OpenCode](https://opencode.ai/docs/)
realizzerebbe ciascun contratto. **Nessuna riga di produzione per OpenCode esiste
in questo repository.** Le informazioni vengono dalla documentazione ufficiale,
consultata il 2026-11-04; le fonti sono citate per ogni punto.

Legenda: **[doc]** = documentato nella pagina citata; **[da verificare]** = non
trovato nella documentazione, o riportato in modo ambiguo: va provato a mano su un
OpenCode installato prima di scriverlo nel codice.

Nota di versione: la pagina di installazione distingue OpenCode V1 e V2 (stesso
eseguibile `opencode`, non installabili insieme con il comando predefinito).
**[da verificare]** quale versione descrivono le pagine citate e se l'integrazione
deve supportarle entrambe; `detect` leggerà la versione e potrà rifiutare quelle
non testate.

Fonti (tutte su `opencode.ai/docs`): [Config](https://opencode.ai/docs/config/) ·
[Plugins](https://opencode.ai/docs/plugins/) · [CLI](https://opencode.ai/docs/cli/) ·
[MCP servers](https://opencode.ai/docs/mcp-servers/) ·
[Permissions](https://opencode.ai/docs/permissions/) ·
[Agents](https://opencode.ai/docs/agents/) ·
[Commands](https://opencode.ai/docs/commands/) ·
[Skills](https://opencode.ai/docs/skills/) ·
[SDK](https://opencode.ai/docs/sdk/) · [Server](https://opencode.ai/docs/server/) ·
[Troubleshooting](https://opencode.ai/docs/troubleshooting/) ·
[Windows (WSL)](https://opencode.ai/docs/windows-wsl/) ·
[Intro/installazione](https://opencode.ai/docs/).

### Nucleo

- `id()` → `"opencode"`, `display_name()` → `"OpenCode"`.
- **Eseguibile `opencode`** [doc: Intro]. Su Windows le pagine consigliano WSL ma
  OpenCode può girare anche direttamente su Windows [doc: Windows (WSL)]; il target
  Windows di cli-companion va quindi provato (comando `.cmd`/`.exe` dello shim
  npm, `opencode.exe`...) **[da verificare]**; le distro WSL funzionano come per
  Claude Code, con la stessa risoluzione del path assoluto.
- **Versione**: `opencode --version` / `-v` [doc: CLI]. (La pagina Intro dice di
  non sapere come controllarla, quella CLI sì: si usa la CLI.)
- `interactive_command()` → `opencode`, da avviare nella cartella del progetto
  [doc: Intro]. Il corrispondente di `--dangerously-skip-permissions` per l'avvio
  interattivo è il flag `--auto`, che approva ciò che altrimenti chiederebbe
  [doc: Permissions, "Auto mode"] **[da verificare]** che esista anche per la TUI
  (la pagina CLI lo elenca fra i flag di `opencode` e di `opencode run`): se sì,
  `"opencode --auto"`; altrimenti `"opencode"` e le conferme restano all'utente.

### 1. Pacchetto

OpenCode non ha un "plugin installabile" con marketplace come Claude Code: la
configurazione sta in file, quindi il pacchetto è un **insieme di file in una
cartella di configurazione** che l'integrazione sceglie. Due strade, in ordine di
preferenza:

1. **Cartella di configurazione dedicata, via `OPENCODE_CONFIG_DIR`** [doc: Config].
   La variabile indica "una cartella per agenti, comandi, modalità e plugin",
   caricata dopo la globale e `.opencode`. Il pacchetto vive in
   `<dati app>/agent-plugins/opencode/<target>/` (come `plugin_root`) e
   `install` non tocca la configurazione dell'utente: imposta quella variabile
   nell'ambiente dei terminali (e in `WSLENV` su WSL, come le variabili
   `CLI_COMPANION_*`). Il core oggi non prevede variabili d'ambiente specifiche
   di un agente: sarebbe un'estensione di un contratto (vedi "Punti in cui il
   core dovrebbe crescere"), non un `if` nel core.
   Alternativa senza variabili: file globali (punto 2).
2. **Configurazione globale dell'utente** `~/.config/opencode/` (anche
   `%USERPROFILE%\.config\opencode` su Windows) [doc: Troubleshooting, Config].
   `install` scrive file *nostri*, distinguibili, senza modificare quelli
   dell'utente: `plugins/cli-companion-notify.ts` (plugin locale, caricato
   all'avvio da `~/.config/opencode/plugins/` [doc: Plugins]), e un frammento nel
   `opencode.json` per il server MCP. `opencode.json` è del solo utente, quindi
   la scrittura è un merge JSON con una chiave nostra (`mcp.cli-companion`) e la
   rimozione toglie solo quella. In alternativa il file esclusivo
   `OPENCODE_CONFIG` [doc: Config] **[da verificare]** per non toccare quello
   dell'utente.

**Server MCP** [doc: MCP servers]:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "cli-companion": {
      "type": "local",
      "command": ["C:/Program Files/cli-companion/cli-companion.exe", "--mcp"],
      "enabled": true
    }
  }
}
```

`command` è un **array** (a differenza di `.mcp.json` di Claude Code), l'exe usa
slash dritti o `/mnt/c/...` su WSL (`InstallCtx::exe_for`). Il server eredita le
variabili `CLI_COMPANION_*` del terminale **[da verificare]**: la pagina elenca
`environment` per aggiungerne ma non dice se il processo MCP erediti l'ambiente di
OpenCode. Se non lo eredita, va passato `environment` con i nomi (non i valori:
il token è per sessione) e questo richiede un meccanismo `{env:NOME}`
[doc: MCP servers lo mostra solo negli `headers` dei server remoti]
**[da verificare]**. Il campo `timeout` (default 30000 ms) basta: le RPC
rispondono subito.

**Plugin e hook** [doc: Plugins]. Un plugin è un modulo JS/TS che esporta una
funzione `async ({ project, client, $, directory, worktree }) => ({ ...hook })`.
Il corrispondente degli hook di Claude Code è il gestore `event`, che riceve gli
eventi del bus, più `permission.asked`. Il plugin può eseguire comandi con `$`
(shell di Bun). Mappatura:

| Evento cli-companion | Evento OpenCode [doc: Plugins] | Note |
| --- | --- | --- |
| `session-start` | `session.created` | l'id di sessione è nell'evento |
| `finished` | `session.idle` | l'esempio ufficiale delle notifiche usa proprio questo |
| `needs-input` | `permission.asked` (e la domanda all'utente) | **[da verificare]** quali eventi coprano le domande dell'agente (`question`) oltre ai permessi |
| `permission-request` | `permission.asked` | porta il tool richiesto |
| `session-end` | `session.deleted` / `server.*` | **[da verificare]**: non c'è un "fine sessione" documentato; un TUI chiuso potrebbe non emettere nulla |

Il plugin non conosce l'app: esegue `<exe> --notify <evento> opencode` con
`$`, passando lo stdin come JSON costruito dal plugin stesso (il formato lo
decidiamo noi, perché *scriviamo* noi il plugin: `HookEvents::normalize` ne è il
lettore). Questo è un vantaggio rispetto a Claude Code: non si dipende da un formato
altrui. Ma `session.idle` non è un hook "Stop" che può rispondere: **un blocco del
fine turno** (`stop_block_output`) richiede che il plugin legga lo stdout del
processo e rilanci un prompt nella stessa sessione, ad esempio con
`client.session.prompt({ path, body })` [doc: SDK]: lo `stdout` del `--notify` non
è letto da OpenCode. **[da verificare]** se `session.idle` si possa far proseguire
direttamente. Fino a quando non è provato, `finished` funziona come notifica pura e
`stop_block_output` rimanda al rilancio da plugin.

Variabili d'ambiente per `--notify`: il plugin gira nel processo di OpenCode e
`$` eredita il suo ambiente [doc: Plugins, `shell.env` imposta variabili per le
shell]; `CLI_COMPANION_SESSION` e `CLI_COMPANION_TOKEN` (id e segreto di quella sessione) sono quelle del terminale
dell'app, quindi presenti **[da verificare]** (nessuna eccezione se OpenCode è
avviato dal terminale di cli-companion).

**Skill dei preset**: skill con `SKILL.md` e frontmatter `name`/`description`, nome
uguale alla cartella (1-64 caratteri, minuscole, cifre, trattini singoli)
[doc: Skills]. Posizioni: `.opencode/skills/<nome>/SKILL.md`,
`~/.config/opencode/skills/<nome>/` e, per compatibilità, anche
`~/.claude/skills/` e `.claude/skills/` [doc: Skills]. Gli id dei preset
(`task-intake`, `skill-wizard`, `project-setup`) rispettano le regole del nome.
Poiché OpenCode legge anche `~/.claude/skills/`, una skill utente sincronizzata per
Claude Code **sarebbe già visibile**: l'integrazione OpenCode non deve duplicarla
(vedi `user_skills`).

**Versione**: hash di plugin + frammento MCP + skill, formato `<release>-h<hash>`,
come `plugin_version_for` di Claude Code.

**Stato installato** (`is_installed`): lettura dei file nostri (esistono ed hanno la
versione attesa); nessun comando `opencode plugin list` è documentato
**[da verificare]**.

**Rimozione**: cancella solo i file con il nostro nome e la chiave `mcp.cli-companion`.

### 2. Eventi degli hook

- Il formato dello stdin lo definisce il plugin che scriviamo: JSON con
  `session_id`, `type`, `message`, `tool`, `summary`, `transcript_path`... →
  `HookEvent`. Il vantaggio è che **normalize** è semplice e testabile con fixture
  nostre. Il rischio è la deriva fra plugin e lettore: la `version` del pacchetto
  deve includere il testo del plugin.
- Una sessione OpenCode ha un id (`Session` in SDK [doc: SDK, `session.get`]); il
  `session.created` lo porta **[da verificare]** il nome esatto del campo
  (`event.properties.info.id`?).
- `transcript_path`: OpenCode non ha un file di transcript: sessioni e messaggi
  stanno in un database SQLite `opencode.db` [doc: Troubleshooting]. Il campo resta
  `None`; vedi `session_usage`.
- `stop_block_output`: vedi sopra; il testo su stdout è consumato dal plugin, non
  da OpenCode, quindi la riga può essere qualunque cosa il plugin sappia leggere
  (ad esempio il solo motivo).

### 3. Ripresa della sessione

Documentato [doc: CLI]: `opencode --session <id>` / `-s` e `opencode --continue` /
`-c` per la TUI; `opencode run` ha gli stessi flag e `--fork`.

```rust
ResumeSpec {
    command_names: vec!["opencode", "opencode.exe"],   // vedi Nucleo: da verificare su Windows
    strip_flags: vec![
        ResumeFlag { names: vec!["--session", "-s"], takes_value: true },
        ResumeFlag { names: vec!["--continue", "-c"], takes_value: false },
        ResumeFlag { names: vec!["--fork"], takes_value: false },
    ],
    resume_template: "{command} --session {id}",
}
```

Dall'hook `session.created` arriva l'id; `opencode session list --format json`
[doc: CLI] permette di cercarlo a mano durante la verifica.

### 4. Automazione dei worker

`opencode run` esegue "non interattivamente con un prompt" [doc: CLI]: è il
candidato naturale. Ma un worker deve restare un **tab con un processo che vive e
può essere osservato** come per Claude Code, e `opencode run` termina a fine
risposta: **[da verificare]** se è adatto (il worker chiama `complete_task` e
l'hook `finished` bloccato gli fa proseguire il turno; con `run` il processo esce
al primo `idle`). L'alternativa è la TUI con prompt iniziale: la CLI documenta il
flag `--prompt` per la TUI? **[da verificare]**, le pagine citate non lo dicono.

Forma probabile, se `run` si rivela adatto:

```text
opencode run "<bootstrap>" --agent <agente> --model <provider/modello> [--dir <cartella>]
```

Mappatura dei livelli di `PermissionMode`:

| `PermissionMode` | OpenCode |
| --- | --- |
| `Plan` | `--agent plan`: agente primario "Plan", limitato all'analisi, può modificare solo i file Markdown dei piani [doc: Agents] |
| `Default` | `--agent build` (predefinito, tutti i tool) con i permessi di configurazione `ask` [doc: Agents, Permissions] |
| `AcceptEdits` | `build` con `permission.edit: "allow"` e il resto in `ask` (nel file di regole, punto 5) |
| `Bypass` | `--auto`: "approva le richieste che altrimenti chiederebbero; le regole `deny` esplicite valgono ancora" [doc: Permissions, CLI] |

Il server MCP deve essere utilizzabile senza conferme: `permission` nel file di
regole con una regola per i tool MCP. Il nome dei tool MCP in OpenCode è
`<server>_<tool>` (prefisso col nome del server) [doc: MCP servers], quindi
`cli-companion_*`; la sintassi esatta per i tool MCP in `permission` non è mostrata
in Permissions ("MCP tools with a wildcard") **[da verificare]**. Il modello
vede i tool con il prefisso: il prompt di bootstrap dice "Call the get_task tool",
e il modello troverà `cli-companion_get_task` **[da verificare]** (se non basta
si adatta il prompt nella `AutomationRequest`, in inglese, restando agnostico).

`model` è nel formato `provider/modello` [doc: Config, CLI]: contiene `/`, che il
modello validato `[A-Za-z0-9._-]` del core **non ammette**. Questo è un
limite del *core* da risolvere estendendo il contratto (ad esempio un
validatore di modelli per integrazione), non un adattamento nell'integrazione.
Annotalo prima di dichiarare `automation`.

### 5. Permessi dei worker

Il formato nativo è nella chiave `permission` di `opencode.json` o dell'agente
[doc: Permissions]:

```json
{
  "permission": {
    "*": "ask",
    "bash": { "*": "ask", "git *": "allow", "rm *": "deny" },
    "edit": "allow"
  }
}
```

- Valori `allow` | `ask` | `deny`; pattern con `*` e `?`; **l'ultima regola che
  corrisponde vince** (il catch-all `*` va per primo) [doc: Permissions].
- `RuleFormat`: `max_rules` e `max_rule_chars` a scelta nostra; `example`:
  `"git *"` (inglese) per l'allow di `bash`. Una regola dell'utente è quindi un
  comando bash con wildcard; il formato per altri tool (`edit`, `webfetch`)
  richiede una sintassi `tool: pattern` **[da definire]** nell'interfaccia: la
  scelta più semplice è limitare le regole di cli-companion a `bash`.
- `validate_rules`: trim, righe vuote e doppioni scartati, nessun a capo,
  lunghezza massima, e un controllo che il pattern non sia vuoto.
- `prepare`: scrive `<dati app>/worker-permissions/<ws>.json` con la sola chiave
  `permission`, nel formato di path del target, e ritorna il path. Va
  passato a OpenCode come **file di configurazione aggiuntivo** (`OPENCODE_CONFIG`,
  caricato fra la globale e il progetto [doc: Config]) o come contenuto inline
  (`OPENCODE_CONFIG_CONTENT`, precedenza più alta [doc: Config]). Le variabili
  d'ambiente richiedono un meccanismo per impostarle nel comando: in
  PowerShell, cmd e bash la sintassi è diversa. **[da verificare]** se esiste un
  flag `--config`: nelle pagine citate no. È l'altro punto in cui il contratto
  `WorkerAutomation` potrebbe dover crescere (variabili d'ambiente per
  comando).
- `guide()`: italiano; spiega la sintassi a wildcard e l'ordine "l'ultima vince",
  avverte che con `--auto` (bypass) i `deny` valgono ancora ma tutto il resto è
  approvato.

### 6. Preset

`PresetLauncher::command` deve avviare OpenCode con la skill del preset come primo
prompt. OpenCode carica le skill **su richiesta** con il tool `skill`, non con un
`/nome` [doc: Skills]. Due strade:

- **Comando personalizzato**: un file `commands/<id>.md` (o la chiave `command`
  in `opencode.json`) con `template` che dice "load the skill `<id>` and follow it"
  [doc: Commands: frontmatter `description`, `agent`, `subtask`, `model`; il corpo
  è il template; `$ARGUMENTS`]; si invoca come `/<id>` nella TUI. Il comando di
  avvio sarebbe `opencode` con prompt `/<id>` **[da verificare]** (flag `--prompt`
  della TUI?). `opencode run --command <id>` esiste [doc: CLI] ma la pagina
  Commands non lo descrive e `run` non è interattivo.
- **Primo prompt testuale** che chiede di caricare la skill (`skill_path`
  rimane quello della skill).

`skill_path(preset)` → `skills/<id>/SKILL.md` (relativo alla radice del pacchetto,
dentro `IntegrationPackage::files`). Il contenuto è **`preset.skill_text()`
identico**: il frontmatter `name`/`description` dei preset ha già i campi richiesti
e `name` coincide con la cartella; se un preset avesse un `name` diverso dall'id,
sarebbe da correggere nel catalogo (non nell'integrazione).

### 7. Skill utente

OpenCode legge `~/.config/opencode/skills/` **e** `~/.claude/skills/` [doc: Skills].
Opzioni:

- `windows_dir()` → `%USERPROFILE%\.config\opencode\skills`;
  `wsl_home_subpath()` → `.config/opencode/skills`. Le skill finiscono due volte
  se l'utente ha anche Claude Code (il nome coincide, quindi il catalogo di OpenCode
  vedrebbe un doppione **[da verificare]**: come si risolve la collisione).
- Oppure **non dichiarare** `user_skills` e lasciare che OpenCode legga quelle di
  Claude Code. Più semplice, ma funziona solo se Claude Code è installato: è
  una scelta di prodotto.

Raccomandazione: dichiarare il contratto solo dopo aver verificato la collisione.

### 8. Limiti di utilizzo

OpenCode usa le chiavi API/abbonamenti dei provider [doc: Config, `provider`]: non
ha un limite "5 ore / settimanale" proprio. Nessuna fonte documenta un endpoint dei
limiti. **Non dichiarare** `usage_limits` (`None`): il core risponde `unsupported`
e l'interfaccia non mostra nulla. Se in futuro un provider espone i limiti,
sarebbe un contratto *per provider*, non per agente: da progettare allora.

### 9. Consumi della sessione

Il transcript non è un file ma il database `opencode.db` [doc: Troubleshooting];
l'SDK espone `session.messages` [doc: SDK] e il server HTTP `GET
/session/:id/message` [doc: Server]. Le pagine citate **non elencano** i campi dei
token (`tokens.input`, `output`, `cache.read`, ...), che stanno in `types.gen.ts`
dell'SDK e nella specifica OpenAPI `/doc` del server **[da verificare]**.

Un `SessionUsageReader` avrebbe senso solo se si legge il database o si interroga
il server (`opencode serve`): in entrambi i casi non c'è un `transcript_path`
come per Claude Code, quindi il contratto attuale (che prende un path di
transcript) **non si adatta**. Fino ad allora: `None` (`Ok(None)`, nessun errore).

### 10. Profilo di input

Nessuna fonte sui tasti per incollare un'immagine nella TUI di OpenCode
**[da verificare]**: si prova a mano (Ctrl+V su Windows e WSL). Se il Ctrl+V
standard funziona, il profilo resta vuoto (`None`), che è il comportamento
corretto.

### Riepilogo: cosa dichiarerebbe OpenCode

| Contratto | Esito | Perché |
| --- | --- | --- |
| `package` | Sì | plugin locale + `mcp` in `opencode.json` + skill |
| `hook_events` | Sì | formato deciso dal nostro plugin |
| `session_resume` | Sì | `--session <id>` / `--continue` documentati |
| `automation` | Sì, dopo due verifiche | `opencode run` adatto? Modello `provider/modello` vs validazione del core |
| `worker_permissions` | Sì | `permission` documentato; serve un modo per passare il file |
| `presets` | Sì | skill + comando personalizzato; da provare il primo prompt |
| `user_skills` | Da decidere | OpenCode legge già `~/.claude/skills/` |
| `usage_limits` | No | non esiste un limite a finestre |
| `session_usage` | No (per ora) | niente transcript su file; il contratto prende un path |
| `input_profile` | No (per ora) | nessuna regola documentata |

### Punti in cui il core dovrebbe crescere

Emersi dall'esempio, da decidere *prima* di scrivere l'integrazione (il core non
si aggira con un `if`, si estende un contratto):

1. **Modelli `provider/modello`**: il modello validato dal core non ammette `/`
   (`AutomationRequest::model`). Serve una validazione demandata
   all'integrazione.
2. **Variabili d'ambiente per un comando** (`OPENCODE_CONFIG_DIR`,
   `OPENCODE_CONFIG`): `WorkerAutomation::command` ritorna solo una riga di comando.
   Per un agente che si configura con l'ambiente servirebbe un modo di esprimerlo
   in modo valido per PowerShell, cmd e bash.
3. **Consumi senza transcript su file**: `SessionUsageReader` presuppone un file
   (`transcript_path`).

Ognuno è un cambio del core coperto da test di conformità e dalla
`FakeIntegration`, e va fatto con Claude Code che continua a passare.
