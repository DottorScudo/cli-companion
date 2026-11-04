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

## 5. Checklist release

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
