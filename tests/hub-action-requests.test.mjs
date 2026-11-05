import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  canDecide,
  changeValue,
  displayState,
  fieldLabel,
  requestActionLabel,
  requestHeading,
  requestOf,
  requestStateLabel,
} from "../src/lib/hubActionRequests.ts";

const request = (extra = {}) => ({
  id: "m1",
  action: "delete",
  reason: "Obsoleto",
  secretary: "ugo",
  task_title: "Login",
  task_status: "failed",
  dependents: [],
  update: null,
  expires_at: 1000,
  state: "pending",
  resolved_at: null,
  note: null,
  ...extra,
});

describe("displayState", () => {
  it("una richiesta in attesa prima della scadenza resta in attesa", () => {
    assert.equal(displayState(request(), 999), "pending");
  });
  it("una richiesta in attesa alla scadenza o dopo è scaduta", () => {
    assert.equal(displayState(request(), 1000), "expired");
    assert.equal(displayState(request(), 5000), "expired");
  });
  it("gli stati già decisi non cambiano con il tempo", () => {
    for (const state of ["confirmed", "cancelled", "expired", "stale", "failed"]) {
      assert.equal(displayState(request({ state }), 5000), state);
    }
  });
});

describe("canDecide", () => {
  it("solo in attesa e non scaduta", () => {
    assert.equal(canDecide(request(), 999), true);
    assert.equal(canDecide(request(), 1000), false);
    assert.equal(canDecide(request({ state: "confirmed" }), 0), false);
  });
});

describe("etichette", () => {
  it("ogni stato ha la sua etichetta", () => {
    assert.deepEqual(["pending", "confirmed", "cancelled", "expired", "stale", "failed"].map(requestStateLabel), [
      "In attesa",
      "Confermata",
      "Annullata",
      "Scaduta",
      "Non più valida",
      "Non eseguita",
    ]);
  });
  it("le azioni usano le etichette del Tasks Manager", () => {
    assert.equal(requestActionLabel("stop"), "Ferma");
    assert.equal(requestActionLabel("delete"), "Elimina");
    assert.equal(requestActionLabel("delete_cascade"), "Elimina tutti");
    assert.equal(requestActionLabel("mark_done"), "Segna come fatto");
    assert.equal(requestActionLabel("update"), "Modifica");
  });
  it("il titolo dice chi chiede e cosa", () => {
    assert.equal(requestHeading(request()), "@ugo chiede: Elimina");
  });
  it("il titolo di una modifica dice che modifica", () => {
    assert.equal(requestHeading(request({ action: "update" })), "@ugo chiede: Modifica");
  });
  it("ogni campo modificabile ha la sua etichetta", () => {
    assert.deepEqual(["title", "task", "depends_on", "model"].map(fieldLabel), [
      "Titolo",
      "Specifica",
      "Dipendenze",
      "Modello",
    ]);
  });
  it("un campo senza valore si legge con una parola, non vuoto", () => {
    assert.equal(changeValue({ field: "model" }, null), "predefinito");
    assert.equal(changeValue({ field: "depends_on" }, null), "nessuna");
    assert.equal(changeValue({ field: "title" }, null), "—");
    assert.equal(changeValue({ field: "title" }, "Login"), "Login");
  });
});

describe("requestOf", () => {
  it("la scheda ha la richiesta, gli altri messaggi no", () => {
    const r = request();
    assert.equal(requestOf({ event: { kind: "action_request", request: r } }), r);
    assert.equal(requestOf({ event: { kind: "action_request_resolved", request: r } }), null);
    assert.equal(requestOf({ event: { kind: "action_request", request: null } }), null);
    assert.equal(requestOf({ event: null }), null);
  });
});
