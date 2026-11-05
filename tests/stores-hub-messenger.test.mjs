import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  INITIAL_HUB,
  channelSelected,
  createHubStore,
  messageAdded,
  previewLoaded,
  seenPruned,
  settingsLoaded,
  threadLoaded,
  threadSeen,
  threadSelected,
  threadsLoaded,
  viewModeSet,
} from "../src/stores/hub.ts";
import { HUB_SEEN_STORAGE_KEY } from "../src/lib/hubUnread.ts";
import { HUB_VIEW_MODE_STORAGE_KEY } from "../src/lib/hubViewMode.ts";
import { threadIsInView } from "../src/lib/hubNotify.ts";

const summary = (id, extra = {}) => ({
  id,
  channel: "general",
  title: `Thread ${id}`,
  state: "open",
  task_count: 0,
  message_count: 0,
  waiting_user: false,
  owner: null,
  created_at: 1,
  updated_at: 1,
  ...extra,
});
const message = (id, extra = {}) => ({
  id,
  author: { type: "user" },
  kind: "text",
  body: id,
  reply_to: null,
  event: null,
  attachments: [],
  created_at: 1,
  ...extra,
});
const thread = (id, messages = [], extra = {}) => ({
  id,
  channel: "general",
  title: id,
  state: "open",
  task_ids: [],
  proposal_id: null,
  owner: null,
  created_at: 1,
  updated_at: 1,
  summary: null,
  messages,
  ...extra,
});
const settingsView = (extra = {}) => ({
  api_key_set: false,
  enabled: true,
  provider: { base_url: "", model: "" },
  language: "it",
  session: { idle_minutes: 20, max_steps: 8 },
  secretaries: [],
  ...extra,
});
const memory = (initial = {}) => {
  const data = new Map(Object.entries(initial));
  return { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v), data };
};
const on = { ...INITIAL_HUB, enabled: true };
const messenger = { ...on, viewMode: "messenger" };
const opened = (s, id, t = thread(id)) => threadLoaded(threadSelected(s, id), t);

describe("hub store: modalità di vista", () => {
  it("di default per canale; viewModeSet cambia e a vuoto non cambia nulla", () => {
    assert.equal(INITIAL_HUB.viewMode, "channels");
    const s = viewModeSet(on, "messenger");
    assert.equal(s.viewMode, "messenger");
    assert.equal(viewModeSet(s, "messenger"), s);
  });

  it("si legge dallo storage all'avvio e si salva a ogni cambio", () => {
    const storage = memory({ [HUB_VIEW_MODE_STORAGE_KEY]: "messenger" });
    const store = createHubStore(storage);
    assert.equal(store.getState().viewMode, "messenger");
    store.viewModeSet("channels");
    assert.equal(storage.data.get(HUB_VIEW_MODE_STORAGE_KEY), "channels");
  });

  it("valore salvato non valido o storage assente: per canale, e non si scrive altro", () => {
    assert.equal(createHubStore(memory({ [HUB_VIEW_MODE_STORAGE_KEY]: "boh" })).getState().viewMode, "channels");
    const store = createHubStore(null);
    store.viewModeSet("messenger");
    assert.equal(store.getState().viewMode, "messenger");
  });

  it("le altre azioni non riscrivono la preferenza", () => {
    const storage = memory();
    const store = createHubStore(storage);
    store.settingsLoaded(settingsView());
    store.channelSelected("w1");
    assert.equal(storage.data.size, 0);
  });

  it("spegnere l'HUB tiene la modalità e i messaggi visti, scarta le anteprime", () => {
    const busy = threadSeen(
      previewLoaded(viewModeSet(on, "messenger"), "t1", { updatedAt: 1, author: "Tu", text: "x", at: 1 }),
      "t1",
      2,
    );
    const off = settingsLoaded(busy, settingsView({ enabled: false }));
    assert.equal(off.viewMode, "messenger");
    assert.deepEqual(off.lastSeen, { t1: 2 });
    assert.deepEqual(off.previews, {});
    assert.equal(off.enabled, false);
  });
});

describe("hub store: anteprime", () => {
  const preview = (updatedAt, text = "x") => ({ updatedAt, author: "Tu", text, at: 1 });

  it("previewLoaded: con l'HUB acceso salva, con l'HUB spento ignora", () => {
    assert.equal(previewLoaded(on, "t", preview(1)).previews.t.text, "x");
    assert.equal(previewLoaded(INITIAL_HUB, "t", preview(1)), INITIAL_HUB);
  });

  it("la conversazione aperta aggiorna la sua anteprima senza rileggerla (vista messenger)", () => {
    const s = opened(messenger, "t1", thread("t1", [message("m1", { body: "ciao" })], { updated_at: 5 }));
    assert.deepEqual(s.previews.t1, { updatedAt: 5, author: "Tu", text: "ciao", at: 1 });
  });

  it("un messaggio inviato aggiorna l'anteprima", () => {
    const s = opened(messenger, "t1", thread("t1", [message("m1")], { updated_at: 5 }));
    const next = messageAdded(s, "t1", message("m2", { body: "nuovo", created_at: 9 }));
    assert.deepEqual(next.previews.t1, { updatedAt: 9, author: "Tu", text: "nuovo", at: 9 });
  });

  it("una conversazione più vecchia dell'anteprima nota non la sostituisce", () => {
    const s = previewLoaded(messenger, "t1", preview(9, "recente"));
    const next = opened(s, "t1", thread("t1", [message("m1")], { updated_at: 5 }));
    assert.equal(next.previews.t1.text, "recente");
  });

  it("con la vista per canale non cambia nulla (nessuna anteprima)", () => {
    const s = opened(on, "t1", thread("t1", [message("m1")]));
    assert.equal(s.previews, on.previews);
  });
});

describe("hub store: non letti", () => {
  it("al primo avvio i thread letti per la prima volta si considerano letti", () => {
    const store = createHubStore(memory());
    assert.equal(store.getState().seenBaseline, true);
    store.settingsLoaded(settingsView());
    store.threadsLoaded("general", [summary("a", { message_count: 3 })]);
    assert.deepEqual(store.getState().lastSeen, { a: 3 });
    store.threadsLoaded("w1", [summary("b", { channel: "w1", message_count: 2 })]);
    assert.deepEqual(store.getState().lastSeen, { a: 3, b: 2 });
  });

  it("al primo avvio un thread nuovo (hub-update) o una rilettura del canale non è baseline", () => {
    let s = { ...on, seenBaseline: true };
    s = threadsLoaded(s, "general", [summary("a", { message_count: 1 })]);
    s = threadsLoaded(s, "general", [summary("a", { message_count: 1 }), summary("n", { message_count: 4 })]);
    assert.deepEqual(s.lastSeen, { a: 1 });
  });

  it("con dati salvati i thread nuovi restano non letti", () => {
    const store = createHubStore(memory({ [HUB_SEEN_STORAGE_KEY]: JSON.stringify({ a: 1 }) }));
    assert.equal(store.getState().seenBaseline, false);
    store.threadsLoaded("general", [summary("a", { message_count: 1 }), summary("n", { message_count: 4 })]);
    assert.deepEqual(store.getState().lastSeen, { a: 1 });
  });

  it("i messaggi visti si salvano quando cambiano, non a vuoto", () => {
    const storage = memory({ [HUB_SEEN_STORAGE_KEY]: "{}" });
    const store = createHubStore(storage);
    store.threadSeen("a", 2);
    assert.equal(storage.data.get(HUB_SEEN_STORAGE_KEY), '{"a":2}');
    storage.data.set(HUB_SEEN_STORAGE_KEY, "toccato");
    store.threadSeen("a", 2);
    store.threadSeen("a", 1);
    assert.equal(storage.data.get(HUB_SEEN_STORAGE_KEY), "toccato");
  });

  it("threadSeen non torna indietro", () => {
    const s = threadSeen(on, "a", 3);
    assert.equal(threadSeen(s, "a", 2), s);
    assert.equal(threadSeen(s, "a", 4).lastSeen.a, 4);
  });

  it("seenPruned: toglie i thread spariti solo a canali tutti letti", () => {
    const s = threadSeen(threadSeen(threadsLoaded(on, "general", [summary("a")]), "a", 1), "gone", 2);
    assert.equal(seenPruned(s, ["general", "w1"]), s);
    assert.deepEqual(seenPruned(s, ["general"]).lastSeen, { a: 1 });
  });
});

describe("hub store: aprire una chat", () => {
  const viewport = (s, extra = {}) => ({
    focused: true,
    hubShown: true,
    channel: s.channel,
    threadId: s.threadId,
    ...extra,
  });

  it("canale e thread impostati insieme: il thread è in vista (notifiche invariate)", () => {
    let s = threadsLoaded(messenger, "w1", [summary("t9", { channel: "w1" })]);
    s = threadSelected(channelSelected(s, "w1"), "t9");
    assert.equal(s.channel, "w1");
    assert.equal(s.threadId, "t9");
    assert.equal(threadIsInView(viewport(s), { channel: "w1", thread_id: "t9" }), true);
    assert.equal(threadIsInView(viewport(s, { focused: false }), { channel: "w1", thread_id: "t9" }), false);
    assert.equal(threadIsInView(viewport(s), { channel: "general", thread_id: "t9" }), false);
  });

  it("stesso canale: il thread aperto non si svuota finché non si sceglie un altro", () => {
    const s = opened(messenger, "t1", thread("t1", [message("m1")]));
    assert.equal(channelSelected(s, "general"), s);
  });

  it("click su una notifica con la vista messenger: stessa sequenza, apre il thread giusto", () => {
    const s = opened(messenger, "t1", thread("t1", [message("m1")]));
    const next = threadSelected(channelSelected(s, "w1"), "t7");
    assert.equal(next.channel, "w1");
    assert.equal(next.threadId, "t7");
    assert.equal(next.thread, null);
    assert.equal(next.viewMode, "messenger");
  });
});
