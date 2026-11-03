import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { setTimeout as sleep } from "node:timers/promises";
import { TERMINAL_TITLE_MAX, createBusyTracker, sanitizeTerminalTitle } from "../src/lib/terminal.ts";

describe("terminale: sanitize titolo", () => {
  it("trim + collassa spazi", () => {
    assert.equal(sanitizeTerminalTitle("  foo   bar\tbaz  "), "foo bar baz");
  });

  it("ignora stringhe vuote", () => {
    assert.equal(sanitizeTerminalTitle(""), null);
    assert.equal(sanitizeTerminalTitle("   \t  "), null);
  });

  it("tronca a 40 caratteri", () => {
    assert.equal(TERMINAL_TITLE_MAX, 40);
    assert.equal(sanitizeTerminalTitle("x".repeat(50)), "x".repeat(40));
  });
});

describe("terminale: semaforo busy/idle", () => {
  it("busy su output, idle dopo silenzio, notifica solo su cambio", async () => {
    const seen = [];
    const t = createBusyTracker({ idleMs: 20, onChange: (b) => seen.push(b) });
    t.markOutput();
    t.markOutput(); // nessuna doppia notifica
    assert.deepEqual(seen, [true]);
    await sleep(60);
    assert.deepEqual(seen, [true, false]);
    t.dispose();
  });

  it("Invio => busy, exit => idle immediato senza notifiche extra", async () => {
    const seen = [];
    const t = createBusyTracker({ idleMs: 30, onChange: (b) => seen.push(b) });
    t.markEnter();
    assert.deepEqual(seen, [true]);
    t.markIdle();
    assert.deepEqual(seen, [true, false]);
    await sleep(60);
    assert.deepEqual(seen, [true, false]);
    t.dispose();
  });
});
