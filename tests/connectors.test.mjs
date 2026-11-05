import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  KIND_SECTIONS,
  MAX_INSTANCES,
  canCreateInstance,
  canToggle,
  descriptorsOfKind,
  findDescriptor,
  instancesOfConnector,
  instancesOfKind,
  isSecretSet,
  orphanInstances,
  retryIn,
  runnableActions,
  statusView,
} from "../src/lib/connectors.ts";

const descriptor = (id, kind, actions = []) => ({ id, kind, label: id, description: "", version: 1, actions });
const view = (id, connector, secrets = []) => ({
  instance: { id, connector, label: id, enabled: false, config: {}, config_version: 1 },
  secrets,
  status: { instance: id, state: "disabled", last_ok_at: null, last_error: null, next_retry_at: null },
});
const status = (state, extra = {}) => ({
  instance: "con-1-1",
  state,
  last_ok_at: null,
  last_error: null,
  next_retry_at: null,
  ...extra,
});

describe("lib/connectors: raggruppamento", () => {
  const descriptors = [descriptor("posta", "source"), descriptor("chat", "contact"), descriptor("feed", "source")];

  it("le sezioni sono Fonti e Canali di contatto, in quest'ordine", () => {
    assert.deepEqual(
      KIND_SECTIONS.map((s) => [s.kind, s.title]),
      [
        ["source", "Fonti"],
        ["contact", "Canali di contatto"],
      ],
    );
  });

  it("descriptorsOfKind divide per tipo mantenendo l'ordine; findDescriptor cerca per id", () => {
    assert.deepEqual(
      descriptorsOfKind(descriptors, "source").map((d) => d.id),
      ["posta", "feed"],
    );
    assert.deepEqual(
      descriptorsOfKind(descriptors, "contact").map((d) => d.id),
      ["chat"],
    );
    assert.equal(findDescriptor(descriptors, "chat")?.kind, "contact");
    assert.equal(findDescriptor(descriptors, "nope"), undefined);
    assert.deepEqual(descriptorsOfKind([], "source"), []);
  });

  it("instancesOfConnector e instancesOfKind", () => {
    const instances = [view("a", "posta"), view("b", "chat"), view("c", "posta"), view("d", "ignoto")];
    assert.deepEqual(
      instancesOfConnector(instances, "posta").map((i) => i.instance.id),
      ["a", "c"],
    );
    assert.deepEqual(
      instancesOfKind(instances, descriptors, "source").map((i) => i.instance.id),
      ["a", "c"],
    );
  });

  it("orphanInstances: le istanze di un connettore non compilato", () => {
    const instances = [view("a", "posta"), view("d", "ignoto")];
    assert.deepEqual(
      orphanInstances(instances, descriptors).map((i) => i.instance.id),
      ["d"],
    );
    assert.deepEqual(orphanInstances([], descriptors), []);
  });

  it("canCreateInstance si ferma al tetto", () => {
    const full = Array.from({ length: MAX_INSTANCES }, (_, i) => view(`i${i}`, "posta"));
    assert.equal(canCreateInstance(full.slice(1)), true);
    assert.equal(canCreateInstance(full), false);
  });

  it("isSecretSet guarda lo slot, mai un valore", () => {
    const v = view("a", "posta", [
      { slot: "token", set: true },
      { slot: "other", set: false },
    ]);
    assert.equal(isSecretSet(v, "token"), true);
    assert.equal(isSecretSet(v, "other"), false);
    assert.equal(isSecretSet(v, "missing"), false);
  });

  it("runnableActions toglie `test`, che ha il pulsante «Prova connessione»", () => {
    const d = descriptor("posta", "source", [
      { id: "authorize", label: "Autorizza", help: "" },
      { id: "test", label: "Prova", help: "" },
    ]);
    assert.deepEqual(
      runnableActions(d).map((a) => a.id),
      ["authorize"],
    );
  });
});

describe("lib/connectors: stato di salute", () => {
  it("retryIn: secondi, minuti, ore; un istante passato è «a breve»", () => {
    const now = 1_000_000;
    assert.equal(retryIn(now - 5, now), "a breve");
    assert.equal(retryIn(now, now), "a breve");
    assert.equal(retryIn(now + 40_000, now), "tra 40 s");
    assert.equal(retryIn(now + 61_000, now), "tra 2 min");
    assert.equal(retryIn(now + 59 * 60_000, now), "tra 59 min");
    assert.equal(retryIn(now + 61 * 60_000, now), "tra 2 h");
  });

  it("statusView: ogni stato ha etichetta e tono; ok e disabled non hanno dettaglio", () => {
    assert.deepEqual(statusView(status("ok"), 0), { label: "Attivo", tone: "ok", detail: null });
    assert.deepEqual(statusView(status("disabled"), 0), { label: "Disabilitato", tone: "muted", detail: null });
    assert.deepEqual(statusView(status("starting"), 0), { label: "In avvio", tone: "muted", detail: null });
  });

  it("statusView: backoff dice quando si riprova e perché", () => {
    const shown = statusView(
      status("backoff", { last_error: "Servizio non raggiungibile.", next_retry_at: 90_000 }),
      0,
    );
    assert.equal(shown.tone, "warn");
    assert.equal(shown.detail, "Servizio non raggiungibile. · Riprova tra 2 min");
    assert.equal(statusView(status("backoff"), 0).detail, null);
    assert.equal(statusView(status("backoff", { next_retry_at: 5000 }), 0).detail, "Riprova tra 5 s");
  });

  it("statusView: error, needs_auth e unavailable mostrano il messaggio", () => {
    assert.deepEqual(statusView(status("error", { last_error: "Configurazione non valida." }), 0), {
      label: "Errore",
      tone: "error",
      detail: "Configurazione non valida.",
    });
    assert.equal(statusView(status("needs_auth"), 0).label, "Serve una nuova autorizzazione");
    assert.equal(statusView(status("needs_auth"), 0).tone, "warn");
    assert.equal(statusView(status("unavailable"), 0).tone, "error");
    assert.equal(statusView(status("unavailable"), 0).detail, null);
  });

  it("canToggle: un connettore non compilato si può solo spegnere", () => {
    assert.equal(canToggle(false, status("disabled")), true);
    assert.equal(canToggle(false, status("unavailable")), false);
    assert.equal(canToggle(true, status("unavailable")), true);
  });
});
