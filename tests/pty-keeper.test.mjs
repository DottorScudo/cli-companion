import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { MAX_DETACHED_EVENTS, createPtyKeeper } from "../src/lib/ptyKeeper.ts";

const flush = () => new Promise((resolve) => setImmediate(resolve));

/** Backend finto: una sessione per tab, come il registro dei PTY (`DuplicateTab`). */
function fakeBackend() {
  const open = new Map();
  const spawns = [];
  const closed = [];
  let lastId = 0;
  let closeError = null;
  return {
    spawns,
    closed,
    failCloseWith: (err) => (closeError = err),
    /** Spawn manuale: resta in volo finché il test non lo risolve. */
    spawnFor(tabId) {
      return (deliver) =>
        new Promise((resolve, reject) => {
          spawns.push({
            tabId,
            deliver,
            resolve: () => {
              if (open.has(tabId)) return reject(new Error("il tab ha gia' una sessione"));
              lastId += 1;
              open.set(tabId, lastId);
              resolve(lastId);
            },
            reject,
          });
        });
    },
    close: async (id) => {
      if (closeError) throw closeError;
      closed.push(id);
      for (const [tab, sid] of open) if (sid === id) open.delete(tab);
    },
    isOpen: (tabId) => open.has(tabId),
  };
}

/** Rinvio manuale: le chiusure partono solo con `run()`. */
function manualDefer() {
  const queue = [];
  return {
    defer: (fn) => {
      const job = { fn, cancelled: false };
      queue.push(job);
      return () => (job.cancelled = true);
    },
    run: () => {
      for (const job of queue.splice(0)) if (!job.cancelled) job.fn();
    },
  };
}

function setup() {
  const backend = fakeBackend();
  const timer = manualDefer();
  const errors = [];
  const keeper = createPtyKeeper({
    close: backend.close,
    defer: timer.defer,
    onError: (scope, err) => errors.push({ scope, err }),
  });
  return { backend, timer, errors, keeper };
}

describe("ptyKeeper: rimontaggio di un terminale", () => {
  it("un rimontaggio con lo spawn in volo riprende la stessa sessione (niente secondo spawn)", async () => {
    const { backend, timer, keeper } = setup();
    const first = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    first.release();
    const second = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    timer.run();
    backend.spawns[0].resolve();

    assert.equal(backend.spawns.length, 1);
    assert.equal(second.adopted, true);
    assert.equal(await second.session, await first.session);
    await flush();
    assert.deepEqual(backend.closed, []);
    assert.ok(backend.isOpen("tab-1"));
  });

  it("l'output arrivato mentre il tab era sganciato va al nuovo terminale, in ordine", async () => {
    const { backend, keeper } = setup();
    const seenByFirst = [];
    const seen = [];
    const first = keeper.attach("tab-1", (e) => seenByFirst.push(e), backend.spawnFor("tab-1"));
    const { deliver } = backend.spawns[0];
    deliver("a");
    first.release();
    deliver("b");
    deliver("c");
    keeper.attach("tab-1", (e) => seen.push(e), backend.spawnFor("tab-1"));
    deliver("d");

    assert.deepEqual(seenByFirst, ["a"]);
    assert.deepEqual(seen, ["b", "c", "d"]);
  });

  it("gli eventi tenuti da sganciato sono limitati: restano i più recenti", () => {
    const { backend, keeper } = setup();
    const seen = [];
    keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1")).release();
    const { deliver } = backend.spawns[0];
    for (let i = 0; i < MAX_DETACHED_EVENTS + 5; i += 1) deliver(i);
    keeper.attach("tab-1", (e) => seen.push(e), backend.spawnFor("tab-1"));

    assert.equal(seen.length, MAX_DETACHED_EVENTS);
    assert.equal(seen[0], 5);
    assert.equal(seen.at(-1), MAX_DETACHED_EVENTS + 4);
  });

  it("il comando di avvio si inietta una volta sola per sessione", () => {
    const { backend, keeper } = setup();
    const first = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    assert.equal(first.claimLaunch(), true);
    first.release();
    const second = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));

    assert.equal(second.claimLaunch(), false);
    assert.equal(first.claimLaunch(), false);
  });

  it("dopo un doppio montaggio senza iniezione, il terminale rimasto può ancora iniettare", () => {
    const { backend, keeper } = setup();
    const first = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    first.release();
    const second = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));

    assert.equal(first.claimLaunch(), false);
    assert.equal(second.claimLaunch(), true);
  });

  it("il rilascio di un aggancio superato non chiude la sessione ripresa", async () => {
    const { backend, timer, keeper } = setup();
    const first = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    first.release();
    keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    first.release();
    timer.run();
    backend.spawns[0].resolve();
    await flush();

    assert.deepEqual(backend.closed, []);
  });
});

describe("ptyKeeper: chiusura", () => {
  it("senza un nuovo aggancio la sessione si chiude al rinvio, anche se lo spawn era in volo", async () => {
    const { backend, timer, keeper } = setup();
    const lease = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    lease.release();
    lease.release();
    timer.run();
    backend.spawns[0].resolve();
    const id = await lease.session;
    await flush();

    assert.deepEqual(backend.closed, [id]);
    assert.equal(backend.isOpen("tab-1"), false);
  });

  it("niente chiusura prima del rinvio", async () => {
    const { backend, keeper } = setup();
    const lease = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    backend.spawns[0].resolve();
    await lease.session;
    lease.release();
    await flush();

    assert.deepEqual(backend.closed, []);
  });

  it("uno spawn nuovo dello stesso tab parte solo a chiusura precedente finita", async () => {
    const { backend, timer, keeper } = setup();
    const first = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    first.release();
    timer.run();
    const second = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    assert.equal(second.adopted, false);
    await flush();
    assert.equal(backend.spawns.length, 1, "il secondo spawn aspetta la chiusura del primo");

    backend.spawns[0].resolve();
    await flush();
    assert.equal(backend.spawns.length, 2);
    backend.spawns[1].resolve();

    assert.notEqual(await second.session, await first.session);
    assert.ok(backend.isOpen("tab-1"));
  });

  it("tab diversi non si aspettano", async () => {
    const { backend, keeper } = setup();
    keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    keeper.attach("tab-2", () => {}, backend.spawnFor("tab-2"));

    assert.equal(backend.spawns.length, 2);
  });

  it("un errore di chiusura viene segnalato", async () => {
    const { backend, timer, errors, keeper } = setup();
    const failure = new Error("chiusura fallita");
    backend.failCloseWith(failure);
    const lease = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    backend.spawns[0].resolve();
    await lease.session;
    lease.release();
    timer.run();
    await flush();

    assert.deepEqual(errors, [{ scope: "chiusura del terminale", err: failure }]);
  });
});

describe("ptyKeeper: spawn fallito", () => {
  it("rifiuta la sessione e il prossimo aggancio riprova con uno spawn nuovo", async () => {
    const { backend, keeper } = setup();
    const first = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    backend.spawns[0].reject(new Error("workspace non trovato"));
    await assert.rejects(first.session, /workspace non trovato/);
    await flush();

    const second = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    assert.equal(second.adopted, false);
    await flush();
    assert.equal(backend.spawns.length, 2);
  });

  it("uno spawn fallito di un tab sganciato non chiude nulla e non lancia", async () => {
    const { backend, timer, errors, keeper } = setup();
    const lease = keeper.attach("tab-1", () => {}, backend.spawnFor("tab-1"));
    lease.session.catch(() => {});
    lease.release();
    timer.run();
    backend.spawns[0].reject(new Error("troppe sessioni aperte"));
    await flush();

    assert.deepEqual(backend.closed, []);
    assert.deepEqual(errors, []);
  });
});
