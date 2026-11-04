import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { MAX_NOTICES, addNotice, createNoticeStore, removeNotice } from "../src/lib/notices.ts";

const input = (key, extra = {}) => ({ key, level: "error", text: `testo ${key}`, ...extra });

describe("addNotice / removeNotice", () => {
  it("aggiunge in coda e rende sticky solo se richiesto", () => {
    const list = addNotice(addNotice([], input("a"), 1), input("b", { sticky: true }), 2);
    assert.deepEqual(
      list.map((n) => [n.key, n.sticky, n.id]),
      [
        ["a", false, 1],
        ["b", true, 2],
      ],
    );
  });

  it("la stessa chiave sostituisce l'avviso e lo riporta in fondo con un id nuovo", () => {
    let list = addNotice([], input("a"), 1);
    list = addNotice(list, input("b"), 2);
    list = addNotice(list, input("a", { text: "nuovo" }), 3);
    assert.deepEqual(
      list.map((n) => [n.key, n.id, n.text]),
      [
        ["b", 2, "testo b"],
        ["a", 3, "nuovo"],
      ],
    );
  });

  it("oltre il tetto cadono i più vecchi", () => {
    let list = [];
    for (let i = 1; i <= MAX_NOTICES + 2; i++) list = addNotice(list, input(`k${i}`), i);
    assert.equal(list.length, MAX_NOTICES);
    assert.equal(list[0].key, "k3");
    assert.equal(list.at(-1).key, `k${MAX_NOTICES + 2}`);
  });

  it("non muta la lista di partenza", () => {
    const base = addNotice([], input("a"), 1);
    addNotice(base, input("b"), 2);
    removeNotice(base, 1);
    assert.equal(base.length, 1);
  });

  it("removeNotice toglie solo l'id dato", () => {
    const list = addNotice(addNotice([], input("a"), 1), input("b"), 2);
    assert.deepEqual(
      removeNotice(list, 1).map((n) => n.key),
      ["b"],
    );
    assert.equal(removeNotice(list, 99).length, 2);
  });
});

describe("createNoticeStore", () => {
  it("notifica gli ascoltatori a ogni modifica e dà snapshot nuovi", () => {
    const store = createNoticeStore();
    let calls = 0;
    const off = store.subscribe(() => calls++);
    const empty = store.getSnapshot();
    store.push(input("a"));
    assert.equal(calls, 1);
    assert.notEqual(store.getSnapshot(), empty);
    const [first] = store.getSnapshot();
    store.dismiss(first.id);
    assert.equal(calls, 2);
    assert.equal(store.getSnapshot().length, 0);
    off();
    store.push(input("b"));
    assert.equal(calls, 2);
  });

  it("chiudere un id inesistente non notifica", () => {
    const store = createNoticeStore();
    let calls = 0;
    store.subscribe(() => calls++);
    store.dismiss(42);
    assert.equal(calls, 0);
  });

  it("lo stesso avviso ripetuto resta uno solo", () => {
    const store = createNoticeStore();
    store.push(input("a"));
    store.push(input("a"));
    assert.equal(store.getSnapshot().length, 1);
  });
});
