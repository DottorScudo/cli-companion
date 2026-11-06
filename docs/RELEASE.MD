# Release su GitHub (repo + CI Windows + updater)

Questa guida copre, nell'ordine: (1) come predisporre il repository GitHub per un nuovo
owner o un fork, (2) come si pubblica una release, (3) come la CI compila l'eseguibile
Windows, (4) come l'updater (firmato) consuma le release. Per il dettaglio del
meccanismo di update vedi [docs/UPDATER.MD](UPDATER.MD).

Stato attuale: il repository esiste (`necos98/cli-companion`) e `updater.config.json`
punta già a esso; la sezione 1 serve solo a chi riparte da zero (nuovo owner o fork).

## Prerequisiti

- `git` e [GitHub CLI](https://cli.github.com/) installati;
- `gh auth login` (o `gh auth status`) con un account che puo' creare repo;
- in locale: Node LTS + Rust stable (stessi della CI).

## 1. Nuovo owner o fork

Tutti i comandi si lanciano dalla root del repository (PowerShell). Sostituisci `OWNER`
con il tuo username GitHub.

```powershell
# Crea la repo sotto il tuo account, collega il remote (nessun push forzato)
# e pubblica il branch principale.
gh repo create OWNER/cli-companion --public --source=. --remote=origin --push
```

Verifica: `gh repo view OWNER/cli-companion --web`.

ATTENZIONE — ordine importante: `updater.config.json` e' compilato **dentro** il
binario Rust (`include_str!`), quindi il binario della release deve essere
compilato DOPO aver impostato il vero owner (`"owner": "OWNER", "repo": "cli-companion"`):

```powershell
git add updater.config.json
git commit -m "updater: set GitHub owner"
git push
```

Servono inoltre i due secret GitHub `TAURI_SIGNING_PRIVATE_KEY` e
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (vedi [docs/UPDATER.MD](UPDATER.MD)): la chiave
privata non va mai committata.

## 2. Pubblicare una release

```powershell
# allinea la versione (package.json, Cargo.toml e lockfile in un colpo solo) e tagga:
npm run bump -- X.Y.Z
git add -A
git commit -m "vX.Y.Z"
git tag vX.Y.Z
git push
git push origin vX.Y.Z
```

Il push del tag fa partire la CI che compila, firma e crea automaticamente la Release
con installer firmato, `latest.json` e checksum. Controlla l'avanzamento con
`gh run watch` oppure nella tab Actions.

Il `tag_name` deve corrispondere alla versione nei file (`v` iniziale opzionale per
l'updater, ma la CI crea la Release solo su tag `v*`).

## 3. Cosa fa la CI (`.github/workflows/build-windows.yml`)

Runner `windows-latest`, build system reale del progetto:

0. cache Rust (`Swatinem/rust-cache`, `shared-key: release`): le build su tag
   la ripristinano ma non la salvano (`save-if` solo su `main`). Le cache dei
   run su tag non sono visibili ad altri tag, quindi la cache viene scaldata
   da `.github/workflows/cache-rust-main.yml` (push su `main` che tocca
   `Cargo.toml`/`Cargo.lock`/workflow, o avvio manuale): fa solo
   `cargo build --release` con dist segnaposto, senza Release ne' artifact;
1. `npm ci` + `npm test` (`node --test tests/*.test.mjs`);
2. `npx tauri build --ci` con i secret di firma → prima `npm run build`
   (`tsc && vite build`), poi backend Rust in release + bundle NSIS
   (`targets=["nsis"]`; NSIS e' scaricato da tauri-bundler automaticamente,
   niente piu' WiX/MSI) e `<setup>.exe.sig`;
3. genera `latest.json` (manifest del plugin updater, UTF-8 senza BOM; fallisce
   se manca la firma);
4. genera `<asset>.sha256` (singolo hash) per exe e setup;
5. carica exe + setup + `.sig` + `latest.json` + checksum come artifact CI;
6. **solo su tag `v*`**: `gh release create` con gli stessi file.

Asset pubblicati in ogni Release:

| Asset | Note |
|---|---|
| `cli-companion_<ver>_x64-setup.exe` | Installer NSIS (quello scaricato dall'updater) |
| `cli-companion_<ver>_x64-setup.exe.sig` | Firma minisign dell'installer |
| `latest.json` | Manifest letto da `tauri-plugin-updater` |
| `cli-companion.exe` | Eseguibile portable (fallback) |
| `<asset>.sha256` | Checksum per download manuali (non usato dall'updater) |

## 4. Come l'updater consuma le release

Flusso della GUI (`tauri-plugin-updater`, `UpdateCheck.tsx`):

1. `check()` scarica
   `https://github.com/necos98/cli-companion/releases/latest/download/latest.json`
   (endpoint in `src-tauri/tauri.conf.json`) e confronta `version` con
   quella installata;
2. se e' piu' nuova, `downloadAndInstall` scarica il setup NSIS e verifica la
   firma con la `pubkey` in `tauri.conf.json` (firma errata = update rifiutato);
3. l'installer parte in modalita' `passive`, l'app si chiude e si riapre da
   sola. I server MCP delle sessioni agent aperte vengono riavviati.

La CLI `--check-update` (`src-tauri/src/updater.rs`) fa solo un controllo
leggero su `releases/latest` via curl, senza scaricare nulla.

Comandi utili senza GUI:

```powershell
cargo run -p cli-companion -- --check-update   # exit 0 ok / 2 disponibile / 1 errore
cargo run -p cli-companion -- --check-update --json
```

## 5. Note di rilascio

La CI crea la Release con `--generate-notes` (elenco dei commit/PR); le note a mano si
aggiungono dopo con `gh release edit vX.Y.Z --notes-file <file>`. Le novità da riportare
vanno scritte qui sotto, per versione, prima del tag.

### Prossima versione: i Bot-agente

- **Dai preset ai Bot.** Task Intake, Project Setup e Skill Wizard non sono più preset
  di agenti con voci rapide proprie ma **Bot** (`Intake`, `Setup`, `Skill Wizard`),
  più il Bot `General` per il lavoro senza specializzazione. Ogni Bot ha compito, agente,
  modello, effort e skill e si gestisce da Impostazioni › Generale › Bot; ogni task
  della board nomina un Bot (snapshot preso all'accettazione).
- **Migrazione automatica.** Gli avvii predefiniti (globale e dei workspace) e le
  proposte di impostazioni in attesa che puntavano ai vecchi preset passano al Bot
  corrispondente al primo avvio. Le voci `@agente` diventano **Bot derivati**
  (es. “Intake (OpenCode)”), creati una volta sola.
- **Effort solo su Claude Code.** Il livello di sforzo del modello (`low` … `max`) vale
  solo per gli agenti che lo dichiarano, oggi Claude Code; sugli altri è ignorato con
  una nota.
- **Rischio downgrade.** Passando a una versione precedente, `bots.json`,
  `workspaces.json` e `launch_settings.json` migrati risultano più recenti di quanto
  quella versione conosce: vengono messi da parte (rinominati `*.bak-<timestamp>`, mai
  cancellati) e l'app riparte con le impostazioni predefinite. I dati si recuperano
  rimettendo a posto il file dal backup, ma i Bot creati e gli avvii scelti dopo
  l'aggiornamento non sono leggibili dalla versione vecchia.

### Prossima versione: tipi di task dei Bot

- **Bot per tipo di task.** Ogni Bot ha i **Tipi di task** che può eseguire (Lavoro, Review,
  Pianificazione; nessuno = solo terminali interattivi), modificabili da Impostazioni › Bot.
  Un task può avere solo un Bot che ne esegue il tipo: proposte, accettazione, creazione e
  cambio di Bot o di tipo lo verificano e non ripiegano mai su `General`.
- **Bot Reviewer e Planner.** Due nuovi Bot predefiniti, `Reviewer` (review) e `Planner`
  (pianificazione, fascia `strong`, effort `high`). `General` esegue il lavoro; Intake, Setup
  e Skill Wizard restano solo interattivi.
- **Migrazione automatica.** I Bot esistenti (derivati e creati dall'utente) ricevono tutti i
  tipi, così nulla cambia; Reviewer e Planner si aggiungono una volta sola (un Bot cancellato
  non ricompare, un nome già preso diventa «Reviewer 2»). `bots.json` passa alla versione 3:
  una versione precedente dell'app lo mette da parte come `bots.json.bak-<timestamp>`.
- **Segretari dell'HUB.** Vedono solo i Bot che possono pianificare; senza Bot con
  Pianificazione abilitato non possono aprire pianificazioni.
- **Badge dei task.** Review, Pianificazione, verdetto, E2E, Bot e Agente: la mappa è in
  `docs/BOARD.MD`, «Badge dei task».

### Prossima versione: la pool predefinita (domande e bug)

- **Domande sul codice.** Si può chiedere al Coordinatore «dove si imposta il tema?»: apre un
  task di pianificazione in sola lettura con il nuovo Bot **Quick Answer** (fascia `standard`,
  effort `low`, skill `code-answer`), che risponde senza proporre nulla. Nel thread arriva
  «Risposta: <titolo>» con la risposta intera; nel dettaglio del task il badge «Risposta».
- **Bot Bug Fixer.** Nuovo Bot per la correzione dei bug (skill `bug-fix`: riproduce, aggiunge
  un test di regressione, corregge la causa). È `strong`/`high`: il costo medio di un bug sale
  rispetto a `General` con il modello del workspace; si regola in Impostazioni › Generale › Bot.
- **Coordinatore come punto d'ingresso.** Smista le richieste: stato del lavoro dalla board,
  domande al Bot che risponde, modifiche al Bot che pianifica, gestione della coda rimandata a un
  segretario che la gestisce. L'Analista non intervista più chi fa solo una domanda.
- **Chi ha già i file.** `bots.json` passa alla versione 5: Quick Answer e Bug Fixer si creano
  **una volta sola** (un Bot cancellato non ricompare, un nome già preso diventa «Quick Answer 2»);
  il `task` di `General` e `Planner` cambia solo se era ancora il testo del preset. Una versione
  precedente dell'app mette da parte il file come `bots.json.bak-<timestamp>`.
- **I prompt dei segretari non si aggiornano da soli.** Coordinatore e Analista già salvati
  tengono il prompt vecchio: la riga e l'editor del segretario mostrano «È disponibile una
  versione aggiornata: Ripristina predefinito». Il ripristino riporta prompt e tool al preset e
  perde eventuali modifiche fatte a mano.
- **Skill del plugin.** `code-answer` e `bug-fix` arrivano con l'aggiornamento del pacchetto
  dell'agente (prontezza «da aggiornare» finché non lo si aggiorna).

## 6. Checklist release

- [ ] `updater.config.json`: `owner`/`repo` reali
- [ ] versione aggiornata con `npm run bump -- X.Y.Z` (package.json, Cargo.toml e
      lockfile; il test `updater: versioni sincronizzate` fallisce se divergono)
- [ ] `npm test` verde in locale
- [ ] tag `vX.Y.Z` creato e pushato, Release apparsa su GitHub
- [ ] secret `TAURI_SIGNING_PRIVATE_KEY` / `_PASSWORD` presenti su GitHub
- [ ] asset attesi presenti (setup + `.sig` + `latest.json` + exe + `.sha256`)
- [ ] `latest.json` raggiungibile da `releases/latest/download/latest.json`
- [ ] smoke test: da un'installazione precedente, «Controlla aggiornamenti»
      rileva la nuova versione e «Aggiorna e riavvia» la installa
