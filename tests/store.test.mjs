import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { createStore, shallowEqual } from "../src/lib/store.ts";

describe("createStore", () => {
  it("getState riflette subito set e update", () => {
    const store = createStore(1);
    store.set(2);
    assert.equal(store.getState(), 2);
    store.update((n) => n + 1);
    assert.equal(store.getState(), 3);
  });

  it("notifica in modo sincrono a ogni cambiamento", () => {
    const store = createStore({ n: 0 });
    const seen = [];
    store.subscribe(() => seen.push(store.getState().n));
    store.set({ n: 1 });
    store.update(() => ({ n: 2 }));
    assert.deepEqual(seen, [1, 2]);
  });

  it("non notifica con lo stesso stato (set e update)", () => {
    const initial = { n: 0 };
    const store = createStore(initial);
    let calls = 0;
    store.subscribe(() => calls++);
    store.set(initial);
    store.update((s) => s);
    assert.equal(calls, 0);
    store.set({ n: 0 });
    assert.equal(calls, 1);
  });

  it("il confronto è Object.is: NaN è uguale a NaN", () => {
    const store = createStore(NaN);
    let calls = 0;
    store.subscribe(() => calls++);
    store.set(NaN);
    assert.equal(calls, 0);
  });

  it("disiscriversi ferma le notifiche, anche due volte", () => {
    const store = createStore(0);
    let calls = 0;
    const off = store.subscribe(() => calls++);
    store.set(1);
    off();
    off();
    store.set(2);
    assert.equal(calls, 1);
  });

  it("un ascoltatore può disiscriversi durante la notifica", () => {
    const store = createStore(0);
    const calls = [];
    let offSecond = () => {};
    store.subscribe(() => {
      calls.push("a");
      offSecond();
    });
    offSecond = store.subscribe(() => calls.push("b"));
    store.subscribe(() => calls.push("c"));
    store.set(1);
    // "b" si è disiscritto prima del suo turno: non viene chiamato.
    assert.deepEqual(calls, ["a", "c"]);
    store.set(2);
    assert.deepEqual(calls, ["a", "c", "a", "c"]);
  });

  it("un ascoltatore iscritto durante la notifica parte dalla notifica successiva", () => {
    const store = createStore(0);
    const calls = [];
    store.subscribe(() => {
      calls.push("a");
      if (calls.length === 1) store.subscribe(() => calls.push("late"));
    });
    store.set(1);
    assert.deepEqual(calls, ["a"]);
    store.set(2);
    assert.deepEqual(calls, ["a", "a", "late"]);
  });

  it("lo stesso ascoltatore iscritto due volte si annulla una iscrizione alla volta", () => {
    const store = createStore(0);
    let calls = 0;
    const listener = () => calls++;
    const offFirst = store.subscribe(listener);
    store.subscribe(listener);
    store.set(1);
    assert.equal(calls, 2);
    offFirst();
    store.set(2);
    assert.equal(calls, 3);
  });

  it("un ascoltatore che aggiorna lo stato vede lo stato nuovo", () => {
    const store = createStore(0);
    store.subscribe(() => {
      if (store.getState() === 1) store.set(2);
    });
    store.set(1);
    assert.equal(store.getState(), 2);
  });
});

describe("shallowEqual", () => {
  it("primitivi e identità", () => {
    assert.ok(shallowEqual(1, 1));
    assert.ok(shallowEqual("a", "a"));
    assert.ok(shallowEqual(NaN, NaN));
    assert.ok(shallowEqual(null, null));
    assert.ok(!shallowEqual(1, 2));
    assert.ok(!shallowEqual(null, {}));
    assert.ok(!shallowEqual(undefined, null));
  });

  it("array: elemento per elemento con Object.is", () => {
    const item = {};
    assert.ok(shallowEqual([1, item], [1, item]));
    assert.ok(!shallowEqual([1, 2], [1, 2, 3]));
    assert.ok(!shallowEqual([{}], [{}]));
    assert.ok(shallowEqual([], []));
  });

  it("oggetti: chiave per chiave", () => {
    const item = {};
    assert.ok(shallowEqual({ a: 1, b: item }, { b: item, a: 1 }));
    assert.ok(!shallowEqual({ a: 1 }, { a: 1, b: 2 }));
    assert.ok(!shallowEqual({ a: {} }, { a: {} }));
  });

  it("una chiave mancante non equivale a una chiave undefined", () => {
    assert.ok(!shallowEqual({ a: undefined }, { b: undefined }));
  });

  it("array e oggetto non sono mai uguali", () => {
    assert.ok(!shallowEqual([], {}));
    assert.ok(!shallowEqual({}, []));
  });
});
