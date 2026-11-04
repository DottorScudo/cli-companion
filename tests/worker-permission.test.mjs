import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_WORKER_PERMISSION,
  WORKER_PERMISSIONS,
  normalizeWorkerPermission,
  parseWorkerRules,
  rulesToText,
  workerPermissionLabel,
  workerPermissionRank,
} from "../src/lib/workerPermission.ts";

describe("workerPermission", () => {
  it("ordine di permissivita'", () => {
    assert.deepEqual([...WORKER_PERMISSIONS], ["plan", "default", "accept-edits", "bypass"]);
    assert.equal(workerPermissionRank("plan"), 0);
    assert.ok(workerPermissionRank("bypass") > workerPermissionRank("accept-edits"));
  });

  it("etichette italiane per ogni modo", () => {
    assert.equal(workerPermissionLabel("plan"), "Sola lettura (plan)");
    assert.equal(workerPermissionLabel("default"), "Chiedi per ogni azione");
    assert.equal(workerPermissionLabel("accept-edits"), "Approva modifiche ai file");
    assert.equal(workerPermissionLabel("bypass"), "Nessuna conferma (bypass)");
  });

  it("valori sconosciuti ripiegano sul default", () => {
    assert.equal(DEFAULT_WORKER_PERMISSION, "accept-edits");
    assert.equal(normalizeWorkerPermission("bypass"), "bypass");
    assert.equal(normalizeWorkerPermission(undefined), "accept-edits");
    assert.equal(normalizeWorkerPermission("yolo"), "accept-edits");
  });

  it("una regola per riga: trim, righe vuote e doppioni tolti, sintassi lasciata all'agente", () => {
    assert.deepEqual(parseWorkerRules("  Bash(git:*) \r\n\n Edit\nBash(git:*)\n"), ["Bash(git:*)", "Edit"]);
    assert.deepEqual(parseWorkerRules("qualunque forma\n"), ["qualunque forma"]);
    assert.deepEqual(parseWorkerRules(""), []);
  });

  it("le regole tornano a testo una per riga", () => {
    assert.equal(rulesToText(["a", "b"]), "a\nb");
    assert.equal(rulesToText(undefined), "");
  });
});
