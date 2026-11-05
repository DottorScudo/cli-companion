import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  HUB_SEEN_STORAGE_KEY,
  baselineSeen,
  isUnread,
  markSeen,
  openThreadCount,
  pruneSeen,
  readSeen,
  writeSeen,
} from "../src/lib/hubUnread.ts";
import {
  DEFAULT_HUB_VIEW_MODE,
  HUB_VIEW_MODE_STORAGE_KEY,
  parseViewMode,
  readViewMode,
  writeViewMode,
} from "../src/lib/hubViewMode.ts";

const summary = (id, count, extra = {}) => ({ id, channel: "general", message_count: count, ...extra });
const memory = (key, initial) => {
  const data = new Map(initial === undefined ? [] : [[key, initial]]);
  return { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v), data };
};
const blocked = {
  getItem() {
    throw new Error("blocked");
  },
  setItem() {
    throw new Error("blocked");
  },
};

describe("non letti", () => {
  it("una chat è non letta se ha più messaggi di quelli visti", () => {
    assert.equal(isUnread({ a: 2 }, summary("a", 3)), true);
    assert.equal(isUnread({ a: 3 }, summary("a", 3)), false);
    assert.equal(isUnread({ a: 5 }, summary("a", 3)), false);
  });

  it("un thread mai visto è non letto se ha messaggi, letto se è vuoto", () => {
    assert.equal(isUnread({}, summary("a", 1)), true);
    assert.equal(isUnread({}, summary("a", 0)), false);
  });

  it("markSeen non torna indietro e non cambia la mappa se non serve", () => {
    const seen = { a: 3 };
    assert.equal(markSeen(seen, "a", 2), seen);
    assert.equal(markSeen(seen, "a", 3), seen);
    assert.equal(markSeen(seen, "b", 0), seen);
    assert.deepEqual(markSeen(seen, "a", 4), { a: 4 });
    assert.deepEqual(markSeen(seen, "b", 1), { a: 3, b: 1 });
    assert.deepEqual(seen, { a: 3 });
  });

  it("markSeen ignora conteggi non validi", () => {
    const seen = { a: 3 };
    assert.equal(markSeen(seen, "a", -1), seen);
    assert.equal(markSeen(seen, "a", Number.NaN), seen);
  });

  it("baselineSeen: i thread senza voce si considerano letti, gli altri restano", () => {
    const seen = { a: 1 };
    assert.deepEqual(baselineSeen(seen, [summary("a", 5), summary("b", 7)]), { a: 1, b: 7 });
    assert.equal(baselineSeen(seen, [summary("a", 5)]), seen);
    assert.equal(baselineSeen(seen, []), seen);
  });

  it("pruneSeen toglie gli id spariti, solo con tutti i canali letti", () => {
    const threads = { general: [summary("a", 1)], w1: [summary("b", 1)] };
    const seen = { a: 1, b: 1, gone: 4 };
    assert.deepEqual(pruneSeen(seen, threads, ["general", "w1"]), { a: 1, b: 1 });
    assert.equal(pruneSeen(seen, threads, ["general", "w1", "w2"]), seen);
    const clean = { a: 1 };
    assert.equal(pruneSeen(clean, threads, ["general", "w1"]), clean);
  });

  it("pruneSeen con un canale letto ma vuoto toglie tutto ciò che era suo", () => {
    assert.deepEqual(pruneSeen({ gone: 1 }, { general: [] }, ["general"]), {});
  });

  it("openThreadCount: il riassunto, o la conversazione se manca; nulla senza thread aperto", () => {
    const threads = { general: [summary("a", 4)] };
    const read = { id: "a", messages: [1, 2, 3, 4, 5] };
    assert.equal(openThreadCount(threads, "general", null, null), null);
    assert.equal(openThreadCount(threads, "general", "a", null), 4);
    assert.equal(openThreadCount(threads, "general", "a", read), 5);
    assert.equal(openThreadCount({}, "general", "a", read), 5);
    assert.equal(openThreadCount({}, "general", "a", null), null);
    assert.equal(openThreadCount(threads, "general", "a", { id: "z", messages: [1] }), 4);
  });
});

describe("messaggi visti: storage", () => {
  it("nulla di salvato, storage assente o rotto = primo avvio (null)", () => {
    assert.equal(readSeen(null), null);
    assert.equal(readSeen(undefined), null);
    assert.equal(readSeen(memory(HUB_SEEN_STORAGE_KEY)), null);
    assert.equal(readSeen(memory(HUB_SEEN_STORAGE_KEY, "{rotto")), null);
    assert.equal(readSeen(memory(HUB_SEEN_STORAGE_KEY, "[1]")), null);
    assert.equal(readSeen(blocked), null);
  });

  it("tiene solo conteggi interi non negativi", () => {
    const raw = JSON.stringify({ a: 2, b: -1, c: 1.5, d: "x", e: 0 });
    assert.deepEqual(readSeen(memory(HUB_SEEN_STORAGE_KEY, raw)), { a: 2, e: 0 });
  });

  it("scrittura e rilettura; storage bloccato: nessun errore", () => {
    const s = memory(HUB_SEEN_STORAGE_KEY);
    writeSeen(s, { a: 3 });
    assert.deepEqual(readSeen(s), { a: 3 });
    assert.doesNotThrow(() => writeSeen(blocked, { a: 3 }));
    assert.doesNotThrow(() => writeSeen(null, { a: 3 }));
  });
});

describe("modalità di vista: storage", () => {
  it("default per canale", () => {
    assert.equal(DEFAULT_HUB_VIEW_MODE, "channels");
    assert.equal(readViewMode(null), "channels");
    assert.equal(readViewMode(memory(HUB_VIEW_MODE_STORAGE_KEY)), "channels");
  });

  it("valori non validi valgono per canale", () => {
    for (const raw of ["", "Messenger", "chat", "{}", "null"]) {
      assert.equal(readViewMode(memory(HUB_VIEW_MODE_STORAGE_KEY, raw)), "channels");
    }
    assert.equal(parseViewMode(undefined), "channels");
    assert.equal(parseViewMode(3), "channels");
  });

  it("scrive e rilegge la scelta", () => {
    const s = memory(HUB_VIEW_MODE_STORAGE_KEY);
    writeViewMode(s, "messenger");
    assert.equal(readViewMode(s), "messenger");
    writeViewMode(s, "channels");
    assert.equal(readViewMode(s), "channels");
  });

  it("storage bloccato: nessun errore e vale il default", () => {
    assert.equal(readViewMode(blocked), "channels");
    assert.doesNotThrow(() => writeViewMode(blocked, "messenger"));
    assert.doesNotThrow(() => writeViewMode(null, "messenger"));
  });
});
