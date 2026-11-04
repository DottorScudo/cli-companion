// Il frontend non duplica ciò che il backend decide: i nomi delle azioni sono
// quelli dell'enum Rust e i colori di stato hanno un'unica fonte, index.css.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ACTION_LABELS, STATUSES, statusDotClass, statusTextClass } from "../src/lib/board.ts";

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const read = (file) => readFileSync(path.join(root, file), "utf8");

const snakeCase = (name) => name.replace(/(?<!^)([A-Z])/g, "_$1").toLowerCase();

/** Varianti dell'enum Rust `TaskAction`, come le serializza serde (snake_case). */
function rustTaskActions() {
  const body = read("src-tauri/src/board/lifecycle/actions.rs").match(/pub enum TaskAction \{([^}]*)\}/)?.[1];
  assert.ok(body, "enum TaskAction non trovato");
  return [...body.matchAll(/^\s*([A-Z]\w*),/gm)].map((m) => snakeCase(m[1]));
}

describe("contratto della board", () => {
  it("le azioni del frontend sono quelle dell'enum Rust", () => {
    assert.deepEqual(Object.keys(ACTION_LABELS).sort(), rustTaskActions().sort());
  });

  it("i colori di stato sono variabili CSS di index.css, non esadecimali in board.ts", () => {
    assert.doesNotMatch(read("src/lib/board.ts"), /#[0-9a-f]{6}\b/i);
    const css = read("src/index.css");
    for (const status of [...STATUSES, "attention"]) {
      const classes = `${statusTextClass(status)} ${statusDotClass(status)}`;
      for (const [, token] of classes.matchAll(/(?:text|bg)-(status-\w+)/g)) {
        assert.match(css, new RegExp(`--color-${token}:`), `${status}: ${token}`);
      }
    }
  });
});
