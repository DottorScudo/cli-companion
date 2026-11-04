import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { startSubscription, startSubscriptions } from "../src/lib/subscription.ts";

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

describe("startSubscriptions", () => {
  it("ready si risolve solo dopo che tutte le sottoscrizioni hanno risposto", async () => {
    const a = deferredSubscription();
    const b = deferredSubscription();
    const { ready } = startSubscriptions([a.subscribe, b.subscribe], assert.fail);
    let done = false;
    void ready.then(() => (done = true));
    a.resolve();
    await settle();
    assert.equal(done, false, "manca b");
    b.resolve();
    await settle();
    assert.equal(done, true);
  });

  it("senza sottoscrizioni ready è già risolta", async () => {
    await startSubscriptions([], assert.fail).ready;
  });

  it("quando ready si risolve gli unlisten sono già memorizzati", async () => {
    const sub = deferredSubscription();
    const subs = startSubscriptions([sub.subscribe], assert.fail);
    sub.resolve();
    await subs.ready;
    subs.unlisten();
    assert.equal(sub.unlistenCalls(), 1);
  });

  it("un fallimento va a onError, le altre proseguono e ready si risolve comunque", async () => {
    const errors = [];
    const boom = new Error("listen fallito");
    const ok = deferredSubscription();
    const { ready, unlisten } = startSubscriptions([() => Promise.reject(boom), ok.subscribe], (e) => errors.push(e));
    ok.resolve();
    await ready;
    assert.deepEqual(errors, [boom]);
    unlisten();
    assert.equal(ok.unlistenCalls(), 1);
  });

  it("un errore sincrono va a onError e non blocca ready", async () => {
    const errors = [];
    const boom = new Error("fuori da Tauri");
    const { ready } = startSubscriptions(
      [
        () => {
          throw boom;
        },
      ],
      (e) => errors.push(e),
    );
    await ready;
    assert.deepEqual(errors, [boom]);
  });

  it("unlisten prima che le sottoscrizioni si risolvano disiscrive chi arriva dopo", async () => {
    const a = deferredSubscription();
    const b = deferredSubscription();
    const { unlisten, ready } = startSubscriptions([a.subscribe, b.subscribe], assert.fail);
    unlisten();
    assert.equal(a.unlistenCalls() + b.unlistenCalls(), 0);
    a.resolve();
    b.resolve();
    await ready;
    assert.equal(a.unlistenCalls(), 1);
    assert.equal(b.unlistenCalls(), 1);
  });

  it("unlisten dopo la risoluzione disiscrive una volta sola, anche chiamato due volte", async () => {
    const sub = deferredSubscription();
    const { unlisten, ready } = startSubscriptions([sub.subscribe], assert.fail);
    sub.resolve();
    await ready;
    unlisten();
    unlisten();
    assert.equal(sub.unlistenCalls(), 1);
  });
});
