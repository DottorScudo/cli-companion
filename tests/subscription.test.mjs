import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { startSubscription } from "../src/lib/subscription.ts";

/** Sottoscrizione controllabile: `resolve` simula l'arrivo di `listen`. */
function deferredSubscription() {
  let resolve;
  let unlistenCalls = 0;
  const promise = new Promise((r) => (resolve = r));
  return {
    subscribe: () => promise,
    resolve: () => resolve(() => unlistenCalls++),
    unlistenCalls: () => unlistenCalls,
  };
}

const settle = () => new Promise((r) => setImmediate(r));

describe("startSubscription", () => {
  it("annullare dopo la risoluzione disiscrive una volta sola", async () => {
    const sub = deferredSubscription();
    const stop = startSubscription(sub.subscribe, assert.fail);
    sub.resolve();
    await settle();
    stop();
    stop();
    assert.equal(sub.unlistenCalls(), 1);
  });

  it("annullare prima della risoluzione disiscrive appena arriva (StrictMode)", async () => {
    const sub = deferredSubscription();
    const stop = startSubscription(sub.subscribe, assert.fail);
    stop();
    assert.equal(sub.unlistenCalls(), 0);
    sub.resolve();
    await settle();
    assert.equal(sub.unlistenCalls(), 1);
  });

  it("senza annullamento il listener resta attivo", async () => {
    const sub = deferredSubscription();
    startSubscription(sub.subscribe, assert.fail);
    sub.resolve();
    await settle();
    assert.equal(sub.unlistenCalls(), 0);
  });

  it("un rifiuto va a onError e annullare dopo non fa danni", async () => {
    const errors = [];
    const boom = new Error("niente Tauri");
    const stop = startSubscription(
      () => Promise.reject(boom),
      (e) => errors.push(e),
    );
    await settle();
    assert.deepEqual(errors, [boom]);
    assert.doesNotThrow(stop);
  });

  it("un errore sincrono va a onError e non registra nulla", () => {
    const errors = [];
    const stop = startSubscription(
      () => {
        throw new Error("fuori da Tauri");
      },
      (e) => errors.push(e),
    );
    assert.equal(errors.length, 1);
    assert.doesNotThrow(stop);
  });
});
