import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const css = readFileSync(path.join(root, "src", "index.css"), "utf8");

// Regressione "righe bianche nelle tendine": il variant `[&>option]` colpiva
// solo le option figlie dirette della select, quindi optgroup e option annidate
// venivano disegnate con lo stile chiaro di sistema. Il CSS globale deve dare a
// tutte le righe lo sfondo del popover e dichiarare lo schema colori scuro.
describe("select: elenco aperto in tema scuro", () => {
  it("index.css dichiara color-scheme: dark", () => {
    assert.match(css, /color-scheme\s*:\s*dark/);
  });

  it("option e optgroup dentro select usano i token del popover", () => {
    const rule = css.match(/select\s+option\s*,\s*select\s+optgroup\s*\{([^}]*)\}/);
    assert.ok(rule, "regola `select option, select optgroup` mancante");
    assert.match(rule[1], /background-color\s*:\s*var\(--popover\)/);
    assert.match(rule[1], /(?<!-)color\s*:\s*var\(--popover-foreground\)/);
  });
});
