import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  INITIAL_CONNECTORS,
  createConnectorsStore,
  removeInstance,
  removeRoute,
  setDescriptors,
  setInstances,
  setRoutes,
  setStatuses,
  statusOf,
  upsertInstance,
  upsertRoute,
} from "../src/stores/connectors.ts";

const status = (instance, state = "disabled", extra = {}) => ({
  instance,
  state,
  last_ok_at: null,
  last_error: null,
  next_retry_at: null,
  ...extra,
});
const view = (id, state = "disabled", extra = {}) => ({
  instance: { id, connector: "posta", label: id, enabled: false, config: {}, config_version: 1 },
  secrets: [{ slot: "api_token", set: false }],
  status: status(id, state, extra),
});
const descriptor = (id) => ({ id, kind: "source", label: id, version: 1 });

describe("stores/connectors: transizioni pure", () => {
  it("setInstances sostituisce tutto, prende gli stati dalle istanze e segna come letto", () => {
    const next = setInstances(INITIAL_CONNECTORS, [view("con-1-1", "ok"), view("con-1-2")]);
    assert.equal(next.loaded, true);
    assert.deepEqual(Object.keys(next.statuses), ["con-1-1", "con-1-2"]);
    assert.equal(next.statuses["con-1-1"].state, "ok");
    const again = setInstances(next, [view("con-1-2")]);
    assert.deepEqual(Object.keys(again.statuses), ["con-1-2"]);
    assert.equal(again.instances.length, 1);
  });

  it("setDescriptors sostituisce i descrittori e lascia il resto", () => {
    const next = setDescriptors(INITIAL_CONNECTORS, [descriptor("posta")]);
    assert.equal(next.descriptors.length, 1);
    assert.equal(next.loaded, false);
    assert.deepEqual(next.instances, []);
  });

  it("upsertInstance sostituisce per id (stesso ordine) o aggiunge in coda, e aggiorna lo stato", () => {
    let s = setInstances(INITIAL_CONNECTORS, [view("con-1-1"), view("con-1-2")]);
    s = upsertInstance(s, view("con-1-1", "ok"));
    assert.deepEqual(
      s.instances.map((i) => i.instance.id),
      ["con-1-1", "con-1-2"],
    );
    assert.equal(s.statuses["con-1-1"].state, "ok");
    s = upsertInstance(s, view("con-1-3"));
    assert.deepEqual(
      s.instances.map((i) => i.instance.id),
      ["con-1-1", "con-1-2", "con-1-3"],
    );
  });

  it("removeInstance toglie istanza e stato; un id sconosciuto non cambia nulla", () => {
    const s = setInstances(INITIAL_CONNECTORS, [view("con-1-1"), view("con-1-2")]);
    const next = removeInstance(s, "con-1-1");
    assert.deepEqual(
      next.instances.map((i) => i.instance.id),
      ["con-1-2"],
    );
    assert.deepEqual(Object.keys(next.statuses), ["con-1-2"]);
    assert.equal(removeInstance(next, "con-9-9"), next);
  });

  it("setStatuses sostituisce l'elenco; lo stesso contenuto non cambia lo stato", () => {
    const s = setInstances(INITIAL_CONNECTORS, [view("con-1-1"), view("con-1-2")]);
    const next = setStatuses(s, [status("con-1-1", "backoff", { next_retry_at: 5 })]);
    assert.equal(next.statuses["con-1-1"].state, "backoff");
    assert.equal("con-1-2" in next.statuses, false, "l'evento porta l'elenco intero");
    assert.equal(setStatuses(next, [status("con-1-1", "backoff", { next_retry_at: 5 })]), next);
  });

  it("statusOf: lo stato ricevuto, altrimenti quello letto con l'istanza, altrimenti null", () => {
    let s = setInstances(INITIAL_CONNECTORS, [view("con-1-1", "ok")]);
    s = setStatuses(s, []);
    assert.equal(statusOf(s, "con-1-1")?.state, "ok");
    s = setStatuses(s, [status("con-1-1", "needs_auth")]);
    assert.equal(statusOf(s, "con-1-1")?.state, "needs_auth");
    assert.equal(statusOf(s, "con-9-9"), null);
  });
});

describe("stores/connectors: archivio", () => {
  it("notifica solo quando qualcosa cambia", () => {
    const store = createConnectorsStore();
    let calls = 0;
    store.subscribe(() => calls++);
    store.descriptorsSet([descriptor("posta")]);
    store.instancesSet([view("con-1-1")]);
    store.statusesSet([status("con-1-1", "ok")]);
    assert.equal(calls, 3);
    store.statusesSet([status("con-1-1", "ok")]);
    store.instanceRemoved("con-9-9");
    assert.equal(calls, 3);
    store.instanceUpserted(view("con-1-2"));
    store.instanceRemoved("con-1-2");
    assert.equal(calls, 5);
    const state = store.getState();
    assert.equal(state.loaded, true);
    assert.equal(state.descriptors[0].id, "posta");
  });
});

describe("stores/connectors: regole", () => {
  const route = (id, instance = "con-1-1") => ({ id, instance, label: id, enabled: true });

  it("setRoutes sostituisce l'elenco; upsertRoute sostituisce per id o aggiunge in coda; removeRoute toglie", () => {
    let s = setRoutes(INITIAL_CONNECTORS, [route("rte-1"), route("rte-2")]);
    s = upsertRoute(s, { ...route("rte-1"), label: "nuova" });
    assert.deepEqual(
      s.routes.map((r) => [r.id, r.label]),
      [
        ["rte-1", "nuova"],
        ["rte-2", "rte-2"],
      ],
    );
    s = upsertRoute(s, route("rte-3"));
    assert.deepEqual(
      s.routes.map((r) => r.id),
      ["rte-1", "rte-2", "rte-3"],
    );
    const next = removeRoute(s, "rte-2");
    assert.deepEqual(
      next.routes.map((r) => r.id),
      ["rte-1", "rte-3"],
    );
    assert.equal(removeRoute(next, "rte-9"), next);
  });

  it("eliminare un'istanza toglie anche le sue regole", () => {
    let s = setInstances(INITIAL_CONNECTORS, [view("con-1-1"), view("con-1-2")]);
    s = setRoutes(s, [route("rte-1", "con-1-1"), route("rte-2", "con-1-2")]);
    const next = removeInstance(s, "con-1-1");
    assert.deepEqual(
      next.routes.map((r) => r.id),
      ["rte-2"],
    );
  });

  it("l'archivio notifica le regole solo quando cambiano", () => {
    const store = createConnectorsStore();
    let calls = 0;
    store.subscribe(() => calls++);
    store.routeUpserted(route("rte-1"));
    store.routeRemoved("rte-9");
    store.routeRemoved("rte-1");
    store.routesSet([route("rte-2")]);
    assert.equal(calls, 3);
    assert.equal(store.getState().routes.length, 1);
  });
});
