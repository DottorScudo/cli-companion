// Contratto sui tipi generati dal Rust (src/bindings, ts-rs): nessuna copia
// scritta a mano di un DTO e nessun import di valore dai bindings nei moduli
// di src/lib (li importano direttamente i test `node --test`, che con il type
// stripping eliminano solo gli import di tipo).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const SRC = path.join(root, "src");
const BINDINGS = path.join(SRC, "bindings");
const LIB = path.join(SRC, "lib");

const TS_FILE = /\.(ts|tsx)$/;

// Omonimi legittimi: tipi del frontend che non descrivono il filo. `Notice` e'
// il toast dell'interfaccia (notices.ts); il `Notice` del Rust si importa come
// `TaskNotifyEvent`.
const HOMONYMS = new Set(["src/lib/notices.ts:Notice"]);

/** File `.ts`/`.tsx` sotto `dir` (percorsi assoluti). */
function sourceFiles(dir) {
  const files = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) files.push(...sourceFiles(full));
    else if (TS_FILE.test(entry.name)) files.push(full);
  }
  return files;
}

/** Nomi dei tipi generati: un file per tipo. */
const generated = readdirSync(BINDINGS)
  .filter((name) => name.endsWith(".ts"))
  .map((name) => name.slice(0, -".ts".length));

const rel = (file) => path.relative(root, file).replaceAll("\\", "/");

describe("bindings: nessuna copia scritta a mano di un DTO", () => {
  it("ci sono tipi generati", () => {
    assert.ok(generated.length > 0, "src/bindings e' vuota: esegui `npm run bindings`");
  });

  it("nessun file fuori da src/bindings dichiara un tipo con il nome di un DTO", () => {
    const declaration = new RegExp(
      `^\\s*(?:export\\s+)?(?:interface\\s+(${generated.join("|")})\\b|type\\s+(${generated.join("|")})\\s*(?:<[^>]*>)?=)`,
    );
    const offenders = [];
    for (const file of sourceFiles(SRC)) {
      if (file.startsWith(BINDINGS + path.sep)) continue;
      readFileSync(file, "utf8")
        .split(/\r?\n/)
        .forEach((line, i) => {
          // I re-export (`export type { X } from ...`) non dichiarano nulla.
          const match = declaration.exec(line);
          const name = match?.[1] ?? match?.[2];
          if (name && !HOMONYMS.has(`${rel(file)}:${name}`)) offenders.push(`${rel(file)}:${i + 1} dichiara ${name}`);
        });
    }
    assert.deepEqual(offenders, [], `usa il tipo generato in src/bindings:\n${offenders.join("\n")}`);
  });
});

describe("bindings: import in src/lib", () => {
  it("sono sempre di tipo (mai di valore)", () => {
    // Import o re-export che nomina un modulo dei bindings, anche su piu' righe.
    const statement = /^\s*(import|export)\b([^;]*?)\bfrom\s+["'][^"']*\/bindings\/[^"']*["']/gm;
    const offenders = [];
    for (const file of sourceFiles(LIB)) {
      const text = readFileSync(file, "utf8");
      for (const match of text.matchAll(statement)) {
        if (!/^\s*type\b/.test(match[2])) {
          const line = text.slice(0, match.index).split("\n").length;
          offenders.push(`${rel(file)}:${line} ${match[0].trim().replace(/\s+/g, " ")}`);
        }
      }
    }
    assert.deepEqual(offenders, [], `usa \`import type\` / \`export type\`:\n${offenders.join("\n")}`);
  });
});
