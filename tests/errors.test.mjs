import { afterEach, beforeEach, describe, it, mock } from "node:test";
import assert from "node:assert/strict";
import { MAX_DETAIL_LENGTH, errorMessage, failureText, ignoreError, report } from "../src/lib/errors.ts";
import { noticeStore } from "../src/lib/notices.ts";

describe("errorMessage", () => {
  it("accetta stringhe (i comandi Tauri rifiutano così), Error e oggetti con message", () => {
    assert.equal(errorMessage("  file bloccato  "), "file bloccato");
    assert.equal(errorMessage(new Error("boom")), "boom");
    assert.equal(errorMessage({ message: "dal backend" }), "dal backend");
    assert.equal(errorMessage(42), "42");
  });

  it("vuoto, null e undefined danno un testo generico", () => {
    for (const empty of ["", "   ", null, undefined, new Error("")]) {
      assert.equal(errorMessage(empty), "errore sconosciuto");
    }
  });

  it("tronca i dettagli troppo lunghi, anche con unicode", () => {
    const long = errorMessage("è".repeat(MAX_DETAIL_LENGTH * 2));
    assert.equal(long.length, MAX_DETAIL_LENGTH + 1);
    assert.ok(long.endsWith("…"));
    assert.equal(errorMessage("è".repeat(MAX_DETAIL_LENGTH)).length, MAX_DETAIL_LENGTH);
  });
});

describe("failureText", () => {
  it("mette l'iniziale maiuscola all'ambito e aggiunge il dettaglio", () => {
    assert.equal(failureText("salvataggio della sessione", "disco pieno"), "Salvataggio della sessione: disco pieno");
  });
});

describe("report / ignoreError", () => {
  let errorLog;
  let debugLog;
  beforeEach(() => {
    errorLog = mock.method(console, "error", () => {});
    debugLog = mock.method(console, "debug", () => {});
    noticeStore.getSnapshot().forEach((n) => noticeStore.dismiss(n.id));
  });
  afterEach(() => mock.restoreAll());

  it("senza notify scrive solo nel log", () => {
    report("scrittura sul terminale", "pty chiuso");
    assert.equal(errorLog.mock.callCount(), 1);
    assert.deepEqual(errorLog.mock.calls[0].arguments, ["[scrittura sul terminale]", "pty chiuso"]);
    assert.equal(noticeStore.getSnapshot().length, 0);
  });

  it("con notify mostra anche un avviso di errore", () => {
    report("salvataggio della sessione", new Error("disco pieno"), { notify: true });
    assert.equal(errorLog.mock.callCount(), 1);
    const [notice] = noticeStore.getSnapshot();
    assert.equal(notice.level, "error");
    assert.equal(notice.text, "Salvataggio della sessione: disco pieno");
  });

  it("lo stesso errore ripetuto non accumula avvisi", () => {
    for (let i = 0; i < 3; i++) report("salvataggio della sessione", "disco pieno", { notify: true });
    assert.equal(noticeStore.getSnapshot().length, 1);
  });

  it("ignoreError non lancia, non avvisa e lascia traccia nel log di debug", () => {
    const handler = ignoreError("rete assente");
    assert.doesNotThrow(() => handler(new Error("x")));
    assert.equal(errorLog.mock.callCount(), 0);
    assert.equal(debugLog.mock.callCount(), 1);
    assert.match(debugLog.mock.calls[0].arguments[0], /rete assente/);
    assert.equal(noticeStore.getSnapshot().length, 0);
  });

  it("ignoreError rifiuta un motivo vuoto", () => {
    assert.throws(() => ignoreError("  "), /motivo/);
  });

  it("funziona come gestore di .catch", async () => {
    await assert.doesNotReject(Promise.reject(new Error("x")).catch(ignoreError("test")));
  });
});
