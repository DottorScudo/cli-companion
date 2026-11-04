// Rigenera i tipi TypeScript dei DTO Rust: `npm run bindings`.
// ts-rs scrive `src/bindings/<Tipo>.ts` durante `cargo test` (un test `export_bindings_<tipo>`
// per ogni tipo con `#[ts(export)]`); la cartella di destinazione e' in `src-tauri/.cargo/config.toml`.
// La cartella viene svuotata prima, cosi' i file dei tipi rimossi non restano orfani.
import { spawnSync } from "node:child_process";
import { rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

const BINDINGS_DIR = path.join(ROOT, "src", "bindings");
const TAURI_DIR = path.join(ROOT, "src-tauri");
const MANIFEST = path.join(TAURI_DIR, "Cargo.toml");
const CARGO = "cargo";
// Filtro per nome: seleziona solo i test di export generati da ts-rs.
const EXPORT_TEST_FILTER = "export_bindings";

rmSync(BINDINGS_DIR, { recursive: true, force: true });

// Nessuna shell e nessun input interpolato: gli argomenti viaggiano come array.
// cwd = src-tauri: cargo cerca `.cargo/config.toml` (TS_RS_EXPORT_DIR) risalendo dalla cwd,
// non dalla cartella del manifest.
const result = spawnSync(CARGO, ["test", "--manifest-path", MANIFEST, EXPORT_TEST_FILTER], {
  stdio: "inherit",
  cwd: TAURI_DIR,
});

if (result.error) {
  console.error(`Impossibile eseguire ${CARGO}: ${result.error.message}`);
  process.exit(1);
}
if (result.status !== 0) {
  process.exit(result.status ?? 1);
}
