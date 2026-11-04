import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { addTab, createTabsStore, findTab, mergeTabs, nextBotName, removeTab } from "../src/stores/tabs.ts";

const tab = (key, extra = {}) => ({
  key,
  workspaceId: "w1",
  command: "",
  startCommand: "",
  bot: `bot-${key}`,
  ...extra,
});
const spec = (key, extra = {}) => ({ key, workspaceId: "w1", command: "", startCommand: "", ...extra });
const keys = (s) => s.map((t) => t.key);
const POOL = ["Bolt", "Nova", "Zed"];

describe("tabs: transizioni pure", () => {
  it("addTab accoda e con chiave già presente ritorna lo stesso stato", () => {
    const s = addTab([], tab("a"));
    assert.deepEqual(keys(s), ["a"]);
    assert.equal(addTab(s, tab("a", { command: "altro" })), s);
    assert.deepEqual(keys(addTab(s, tab("b"))), ["a", "b"]);
  });

  it("removeTab toglie solo quella chiave; assente = stesso stato", () => {
    const s = [tab("a"), tab("b"), tab("c")];
    assert.deepEqual(keys(removeTab(s, "b")), ["a", "c"]);
    assert.equal(removeTab(s, "zzz"), s);
  });

  it("findTab", () => {
    const s = [tab("a"), tab("b")];
    assert.equal(findTab(s, "b"), s[1]);
    assert.equal(findTab(s, "x"), undefined);
  });

  it("mergeTabs mantiene l'ordine esistente e salta le chiavi già presenti", () => {
    const existing = [tab("w1"), tab("w2")];
    const merged = mergeTabs(existing, [tab("r1"), tab("w2", { command: "duplicato" }), tab("r2")]);
    assert.deepEqual(keys(merged), ["w1", "w2", "r1", "r2"]);
    assert.equal(merged[0], existing[0]);
    assert.equal(merged[1], existing[1], "il tab presente non viene sostituito");
  });

  it("mergeTabs salta anche i duplicati dentro l'elenco in ingresso", () => {
    assert.deepEqual(keys(mergeTabs([], [tab("a"), tab("a"), tab("b")])), ["a", "b"]);
  });

  it("mergeTabs senza chiavi nuove ritorna lo stesso stato", () => {
    const s = [tab("a")];
    assert.equal(mergeTabs(s, [tab("a")]), s);
    assert.equal(mergeTabs(s, []), s);
  });

  it("nextBotName evita i nomi dei bot presenti, senza badare alle maiuscole", () => {
    const s = [tab("a", { bot: "Bolt" }), tab("b", { bot: "nova" })];
    assert.equal(
      nextBotName(s, POOL, () => 0),
      "Zed",
    );
  });
});

describe("createTabsStore", () => {
  it("open assegna nomi bot unici anche per chiamate consecutive", () => {
    const store = createTabsStore(() => 0);
    const names = ["a", "b", "c"].map((key) => store.open(spec(key), POOL).bot);
    assert.equal(new Set(names).size, 3);
    assert.deepEqual([...names].sort(), [...POOL].sort());
  });

  it("a pool esaurita i nomi restano unici (suffisso numerico)", () => {
    const store = createTabsStore(() => 0);
    const names = ["a", "b", "c", "d"].map((key) => store.open(spec(key), ["Bolt"]).bot);
    assert.equal(new Set(names.map((n) => n.toLowerCase())).size, 4);
  });

  it("open con chiave già presente ritorna null e non notifica", () => {
    const store = createTabsStore();
    store.open(spec("a"), POOL);
    let calls = 0;
    store.subscribe(() => calls++);
    assert.equal(store.open(spec("a", { workspaceId: "w2", command: "x", startCommand: "x" }), POOL), null);
    assert.equal(calls, 0);
    assert.equal(store.getState().length, 1);
  });

  it("open è sincrono: getState e get vedono subito il tab, che si accoda in fondo", () => {
    const store = createTabsStore();
    store.open(spec("a"), POOL);
    const opened = store.open(spec("b", { command: "claude", startCommand: "claude" }), POOL);
    assert.equal(store.get("b"), opened);
    assert.deepEqual(keys(store.getState()), ["a", "b"]);
    assert.equal(opened.command, "claude");
  });

  it("remove e merge notificano solo se cambia qualcosa", () => {
    const store = createTabsStore();
    store.open(spec("a"), POOL);
    let calls = 0;
    store.subscribe(() => calls++);
    store.remove("nope");
    store.merge([store.get("a")]);
    assert.equal(calls, 0);
    store.merge([tab("r1")]);
    store.remove("a");
    assert.equal(calls, 2);
    assert.deepEqual(keys(store.getState()), ["r1"]);
  });
});
