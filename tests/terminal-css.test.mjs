import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

// Regressione "freccetta scrollbar in basso a destra": xterm.js v6 usa uno
// slider custom senza frecce, ma `.xterm-viewport` (overflow-y: scroll)
// mostra anche i pulsanti nativi del motore. Il CSS dell'app deve tenerli
// nascosti dentro i pannelli terminale, senza toccare le meccaniche di scroll.
describe("terminale: chrome scrollbar nativa", () => {
  it("index.css nasconde i pulsanti webkit dentro .terminal-pane", () => {
    const css = readFileSync(path.join(root, "src", "index.css"), "utf8");
    const rule = css.match(/\.terminal-pane\s*::-webkit-scrollbar-button\s*\{([^}]*)\}/);
    assert.ok(rule, "regola .terminal-pane ::-webkit-scrollbar-button mancante");
    assert.match(rule[1], /display\s*:\s*none/, "il pulsante deve essere display:none");
  });

  it("non nasconde l'intera scrollbar (track/slider devono restare)", () => {
    const css = readFileSync(path.join(root, "src", "index.css"), "utf8");
    assert.ok(
      !/\.terminal-pane[^{]*\{[^}]*scrollbar-width\s*:\s*none/.test(css),
      "scrollbar-width:none toglierebbe anche lo slider: usare solo ::-webkit-scrollbar-button",
    );
  });
});
