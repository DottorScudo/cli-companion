import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  INITIAL_WORKSPACES,
  added,
  createWorkspacesStore,
  failed,
  loaded,
  removed,
  reordered,
  updated,
  visibleWorkspaces,
  workspaceById,
} from "../src/stores/workspaces.ts";

const ws = (id, extra = {}) => ({ id, name: `Workspace ${id}`, ...extra });
const ids = (list) => list.map((w) => w.id);

describe("workspaces: transizioni", () => {
  it("stato iniziale: in caricamento, senza errore", () => {
    assert.deepEqual(INITIAL_WORKSPACES, { list: [], status: "loading", error: null });
  });

  it("loaded e failed", () => {
    const list = [ws("a")];
    assert.deepEqual(loaded(INITIAL_WORKSPACES, list), { list, status: "ready", error: null });
    const err = failed(INITIAL_WORKSPACES, "boom");
    assert.equal(err.status, "error");
    assert.equal(err.error, "boom");
    assert.equal(err.list, INITIAL_WORKSPACES.list);
  });

  it("added accoda", () => {
    const s = added(loaded(INITIAL_WORKSPACES, [ws("a")]), ws("b"));
    assert.deepEqual(ids(s.list), ["a", "b"]);
  });

  it("updated sostituisce per id; id sconosciuto o stesso oggetto = stesso stato", () => {
    const a = ws("a");
    const s = loaded(INITIAL_WORKSPACES, [a, ws("b")]);
    const renamed = ws("a", { name: "Nuovo" });
    assert.equal(updated(s, renamed).list[0], renamed);
    assert.equal(updated(s, ws("zzz")), s);
    assert.equal(updated(s, a), s);
  });

  it("removed toglie per id; id assente = stesso stato", () => {
    const s = loaded(INITIAL_WORKSPACES, [ws("a"), ws("b")]);
    assert.deepEqual(ids(removed(s, "a").list), ["b"]);
    assert.equal(removed(s, "zzz"), s);
  });

  it("reordered sostituisce l'elenco; lo stesso elenco = stesso stato", () => {
    const s = loaded(INITIAL_WORKSPACES, [ws("a"), ws("b")]);
    const list = [s.list[1], s.list[0]];
    assert.deepEqual(ids(reordered(s, list).list), ["b", "a"]);
    assert.equal(reordered(s, s.list), s);
  });
});

describe("workspaces: letture", () => {
  it("visibleWorkspaces esclude gli archiviati e ha riferimento stabile per elenco", () => {
    const s = loaded(INITIAL_WORKSPACES, [ws("a"), ws("b", { archived: true }), ws("c")]);
    assert.deepEqual(ids(visibleWorkspaces(s)), ["a", "c"]);
    assert.equal(
      visibleWorkspaces(s),
      visibleWorkspaces({ ...s, status: "error" }),
      "stesso elenco = stesso risultato",
    );
    assert.notEqual(visibleWorkspaces(s), visibleWorkspaces(added(s, ws("d"))));
  });

  it("workspaceById", () => {
    const s = loaded(INITIAL_WORKSPACES, [ws("a"), ws("b")]);
    assert.equal(workspaceById(s, "b"), s.list[1]);
    assert.equal(workspaceById(s, "x"), undefined);
  });
});

describe("createWorkspacesStore", () => {
  it("le azioni a vuoto non notificano", () => {
    const store = createWorkspacesStore();
    store.loaded([ws("a")]);
    let calls = 0;
    store.subscribe(() => calls++);
    store.updated(ws("zzz"));
    store.removed("zzz");
    store.reordered(store.getState().list);
    assert.equal(calls, 0);
    store.added(ws("b"));
    assert.equal(calls, 1);
    assert.equal(store.get("b").id, "b");
  });
});
