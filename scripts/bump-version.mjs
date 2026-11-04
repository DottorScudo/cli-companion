// Unica procedura per cambiare la versione dell'app: `npm run bump -- <x.y.z>`.
// Aggiorna package.json, package-lock.json (radice), src-tauri/Cargo.toml e la voce
// `cli-companion` di src-tauri/Cargo.lock. tauri.conf.json non ha `version`: Tauri
// usa quella di Cargo.toml.
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const CRATE_NAME = "cli-companion";
const VERSION_PATTERN = /^\d+\.\d+\.\d+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$/;

const PACKAGE_JSON = "package.json";
const PACKAGE_LOCK = "package-lock.json";
const CARGO_TOML = "src-tauri/Cargo.toml";
const CARGO_LOCK = "src-tauri/Cargo.lock";

export function assertValidVersion(version) {
  if (typeof version !== "string" || !VERSION_PATTERN.test(version)) {
    throw new Error(`Versione non valida: "${version}" (atteso x.y.z, es. 0.18.0)`);
  }
}

const eolOf = (raw) => (raw.includes("\r\n") ? "\r\n" : "\n");

// Sostituisce l'unica occorrenza di `pattern` (con il gruppo 1 prima del numero di versione).
function replaceOnce(raw, pattern, version, label) {
  const count = (raw.match(new RegExp(pattern, `${pattern.flags}g`)) ?? []).length;
  if (count !== 1) {
    throw new Error(`${label}: attesa 1 occorrenza della versione, trovate ${count}`);
  }
  return raw.replace(pattern, `$1${version}$2`);
}

function rewriteJson(raw, mutate) {
  const eol = eolOf(raw);
  const data = JSON.parse(raw);
  mutate(data);
  return JSON.stringify(data, null, 2).replace(/\n/g, eol) + (raw.endsWith("\n") ? eol : "");
}

export function bumpPackageJson(raw, version) {
  return rewriteJson(raw, (pkg) => {
    pkg.version = version;
  });
}

export function bumpPackageLock(raw, version) {
  return rewriteJson(raw, (lock) => {
    const root = lock.packages?.[""];
    if (!root) throw new Error(`${PACKAGE_LOCK}: voce radice mancante in "packages"`);
    lock.version = version;
    root.version = version;
  });
}

export function bumpCargoToml(raw, version) {
  return replaceOnce(raw, /^(version\s*=\s*")[^"]*(")/m, version, CARGO_TOML);
}

export function bumpCargoLock(raw, version) {
  const entry = new RegExp(`(\\[\\[package\\]\\]\\r?\\nname = "${CRATE_NAME}"\\r?\\nversion = ")[^"]*(")`);
  return replaceOnce(raw, entry, version, CARGO_LOCK);
}

const BUMPS = [
  [PACKAGE_JSON, bumpPackageJson],
  [PACKAGE_LOCK, bumpPackageLock],
  [CARGO_TOML, bumpCargoToml],
  [CARGO_LOCK, bumpCargoLock],
];

// Calcola tutti i nuovi contenuti prima di scrivere, cosi' un errore non lascia file a meta'.
export function bumpVersion(version, root = ROOT) {
  assertValidVersion(version);
  const updates = BUMPS.map(([file, bump]) => {
    const target = path.join(root, file);
    const before = readFileSync(target, "utf8");
    return { file, target, before, after: bump(before, version) };
  });
  const changed = updates.filter((u) => u.after !== u.before);
  for (const { target, after } of changed) writeFileSync(target, after);
  return changed.map((u) => u.file);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const changed = bumpVersion(process.argv[2]);
    console.log(changed.length ? `Aggiornati: ${changed.join(", ")}` : "Nessuna modifica: versione già allineata.");
  } catch (err) {
    console.error(err.message);
    process.exitCode = 1;
  }
}
