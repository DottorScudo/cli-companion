import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  INITIAL_HUB,
  channelSelected,
  createHubStore,
  filterSet,
  messageAdded,
  replySet,
  reservedBotNames,
  secretaryStatusLoaded,
  settingsLoaded,
  summaryUpdated,
  threadLoaded,
  threadRemoved,
  threadSelected,
  threadsLoaded,
} from "../src/stores/hub.ts";
import { questionStates } from "../src/lib/hubQuestions.ts";

const summary = (id, extra = {}) => ({
  id,
  channel: "general",
  title: `Thread ${id}`,
  state: "open",
  task_count: 0,
  message_count: 0,
  waiting_user: false,
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
  created_at: 1,
  updated_at: 1,
  messages,
  ...extra,
});
const secretary = (name, extra = {}) => ({
  name,
  image: null,
  system_prompt: "You help.",
  tools: [],
  channels: { mode: "all" },
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
const status = (secretaryName, extra = {}) => ({
  channel: "general",
  thread_id: "t1",
  secretary: secretaryName,
  expires_at: 1000,
  working: false,
  ...extra,
});
const ids = (list) => list.map((t) => t.id);
const withThread = (t) => threadLoaded(threadSelected({ ...INITIAL_HUB, enabled: true }, t.id), t);

describe("hub store: impostazioni", () => {
  it("spento di default", () => {
    assert.equal(INITIAL_HUB.enabled, false);
    assert.equal(INITIAL_HUB.channel, "general");
    assert.equal(INITIAL_HUB.filter, "open");
  });

  it("senza impostazioni lette non ci sono né impostazioni né segretari svegli", () => {
    assert.equal(INITIAL_HUB.settings, null);
    assert.deepEqual(INITIAL_HUB.secretaryStatus, []);
  });

  it("settingsLoaded: accende l'HUB e tiene le impostazioni", () => {
    const view = settingsView({ secretaries: [secretary("mario")] });
    const on = settingsLoaded(INITIAL_HUB, view);
    assert.equal(on.enabled, true);
    assert.equal(on.settings, view);
  });

  it("settingsLoaded: un nuovo salvataggio sostituisce le impostazioni senza toccare il resto", () => {
    const busy = threadsLoaded(withThread(thread("t1")), "general", [summary("t1")]);
    const next = settingsLoaded(busy, settingsView({ language: "en" }));
    assert.equal(next.settings.language, "en");
    assert.equal(next.thread, busy.thread);
    assert.equal(next.threads, busy.threads);
  });

  it("spegnere l'HUB scarta thread e segretari svegli, ma tiene le impostazioni", () => {
    const busy = secretaryStatusLoaded(threadsLoaded(withThread(thread("t1")), "general", [summary("t1")]), [
      status("mario"),
    ]);
    const view = settingsView({ enabled: false, secretaries: [secretary("mario")] });
    assert.deepEqual(settingsLoaded(busy, view), { ...INITIAL_HUB, settings: view });
  });

  it("HUB spento: le impostazioni si leggono comunque", () => {
    const view = settingsView({ enabled: false });
    const s = settingsLoaded(INITIAL_HUB, view);
    assert.equal(s.enabled, false);
    assert.equal(s.settings, view);
  });
});

describe("hub store: segretari", () => {
  it("secretaryStatusLoaded sostituisce l'elenco intero", () => {
    const on = settingsLoaded(INITIAL_HUB, settingsView());
    const a = secretaryStatusLoaded(on, [status("mario"), status("luisa")]);
    assert.deepEqual(
      a.secretaryStatus.map((x) => x.secretary),
      ["mario", "luisa"],
    );
    assert.deepEqual(secretaryStatusLoaded(a, []).secretaryStatus, []);
  });

  it("secretaryStatusLoaded con l'HUB spento non cambia nulla", () => {
    assert.equal(secretaryStatusLoaded(INITIAL_HUB, [status("mario")]), INITIAL_HUB);
  });

  it("reservedBotNames: i nomi dei segretari; nessuno prima di leggere le impostazioni", () => {
    assert.deepEqual(reservedBotNames(INITIAL_HUB), []);
    const s = settingsLoaded(INITIAL_HUB, settingsView({ secretaries: [secretary("mario"), secretary("bolt")] }));
    assert.deepEqual(reservedBotNames(s), ["mario", "bolt"]);
  });

  it("i nomi riservati restano anche con l'HUB spento", () => {
    const s = settingsLoaded(INITIAL_HUB, settingsView({ enabled: false, secretaries: [secretary("mario")] }));
    assert.deepEqual(reservedBotNames(s), ["mario"]);
  });
});

describe("hub store: canale, filtro, elenchi", () => {
  it("channelSelected azzera thread e risposta; lo stesso canale = stesso stato", () => {
    const s = replySet(withThread(thread("t1", [message("m1")])), "m1");
    assert.equal(channelSelected(s, "general"), s);
    const moved = channelSelected(s, "ws1");
    assert.equal(moved.channel, "ws1");
    assert.equal(moved.threadId, null);
    assert.equal(moved.thread, null);
    assert.equal(moved.replyTo, null);
  });

  it("filterSet", () => {
    assert.equal(filterSet(INITIAL_HUB, "open"), INITIAL_HUB);
    assert.equal(filterSet(INITIAL_HUB, "all").filter, "all");
  });

  it("threadsLoaded sostituisce l'elenco del solo canale", () => {
    const a = threadsLoaded(INITIAL_HUB, "general", [summary("g1")]);
    const b = threadsLoaded(a, "ws1", [summary("w1", { channel: "ws1" })]);
    assert.deepEqual(ids(b.threads.general), ["g1"]);
    assert.deepEqual(ids(b.threads.ws1), ["w1"]);
    assert.deepEqual(ids(threadsLoaded(b, "general", []).threads.general), []);
  });

  it("summaryUpdated: inserisce nel canale del thread e ordina per aggiornamento", () => {
    const s0 = threadsLoaded(INITIAL_HUB, "general", [summary("a", { updated_at: 5 })]);
    const s1 = summaryUpdated(s0, summary("b", { updated_at: 9 }));
    assert.deepEqual(ids(s1.threads.general), ["b", "a"]);
    const s2 = summaryUpdated(s1, summary("w", { channel: "ws1" }));
    assert.deepEqual(ids(s2.threads.ws1), ["w"]);
    assert.equal(s2.threads.general, s1.threads.general);
  });

  it("summaryUpdated: una versione più vecchia non cambia lo stato", () => {
    const s = threadsLoaded(INITIAL_HUB, "general", [summary("a", { updated_at: 5 })]);
    assert.equal(summaryUpdated(s, summary("a", { updated_at: 2 })), s);
  });

  it("summaryUpdated: il waiting_user si aggiorna", () => {
    const s = threadsLoaded(INITIAL_HUB, "general", [summary("a")]);
    const next = summaryUpdated(s, summary("a", { waiting_user: true, updated_at: 2 }));
    assert.equal(next.threads.general[0].waiting_user, true);
  });
});

describe("hub store: conversazione", () => {
  it("threadSelected azzera la conversazione e la risposta; lo stesso id = stesso stato", () => {
    const s = replySet(withThread(thread("t1", [message("m1")])), "m1");
    assert.equal(threadSelected(s, "t1"), s);
    const other = threadSelected(s, "t2");
    assert.equal(other.threadId, "t2");
    assert.equal(other.thread, null);
    assert.equal(other.replyTo, null);
    assert.equal(threadSelected(other, null).threadId, null);
  });

  it("threadLoaded: vale solo per il thread selezionato", () => {
    const sel = threadSelected(INITIAL_HUB, "t1");
    assert.equal(threadLoaded(sel, thread("t2")), sel);
    assert.equal(threadLoaded(INITIAL_HUB, thread("t1")), INITIAL_HUB);
    const t = thread("t1");
    assert.equal(threadLoaded(sel, t).thread, t);
  });

  it("threadLoaded: una lettura più vecchia di quella nota non la sostituisce", () => {
    const fresh = withThread(thread("t1", [message("m1")], { updated_at: 9 }));
    assert.equal(threadLoaded(fresh, thread("t1", [], { updated_at: 4 })), fresh);
    const newer = thread("t1", [message("m1"), message("m2")], { updated_at: 12 });
    assert.equal(threadLoaded(fresh, newer).thread, newer);
  });

  it("messageAdded: aggiunge al thread aperto e chiude la risposta", () => {
    const s = replySet(withThread(thread("t1", [message("m1")])), "m1");
    const next = messageAdded(s, "t1", message("m2", { reply_to: "m1", created_at: 7 }));
    assert.deepEqual(
      next.thread.messages.map((m) => m.id),
      ["m1", "m2"],
    );
    assert.equal(next.thread.updated_at, 7);
    assert.equal(next.replyTo, null);
  });

  it("messageAdded: thread non aperto o messaggio già presente = stesso stato", () => {
    const s = withThread(thread("t1", [message("m1")]));
    assert.equal(messageAdded(s, "altro", message("m2")), s);
    assert.equal(messageAdded(INITIAL_HUB, "t1", message("m2")), INITIAL_HUB);
    assert.equal(messageAdded(s, "t1", message("m1")), s);
  });

  it("risposta a una domanda: l'invio e la rilettura con l'esito portano la scheda a «risposta inviata»", () => {
    const question = message("m1", {
      author: { type: "system" },
      event: {
        kind: "task_question",
        workspace: "ws",
        task_id: "t1",
        question: { id: "q-1-1", text: "Quale?", options: ["A", "B"] },
      },
    });
    const sent = message("m2", { reply_to: "m1", body: "B", created_at: 5 });
    const outcome = message("m3", {
      author: { type: "system" },
      reply_to: "m2",
      body: "Risposta inviata al worker.",
      created_at: 6,
    });
    let s = withThread(thread("t1", [question], { updated_at: 4 }));
    assert.deepEqual(questionStates(s.thread.messages).m1, { kind: "pending" });
    s = messageAdded(s, "t1", sent);
    assert.deepEqual(questionStates(s.thread.messages).m1, { kind: "pending" });
    s = threadLoaded(s, thread("t1", [question, sent, outcome], { updated_at: 6 }));
    assert.deepEqual(questionStates(s.thread.messages).m1, { kind: "answered", answer: "B" });
  });

  it("replySet: inizia e annulla la risposta", () => {
    assert.equal(replySet(INITIAL_HUB, null), INITIAL_HUB);
    const s = replySet(INITIAL_HUB, "m1");
    assert.equal(s.replyTo, "m1");
    assert.equal(replySet(s, "m1"), s);
    assert.equal(replySet(s, null).replyTo, null);
  });
});

describe("createHubStore", () => {
  it("le azioni a vuoto non notificano", () => {
    const store = createHubStore();
    let calls = 0;
    store.subscribe(() => (calls += 1));
    store.channelSelected("general");
    store.filterSet("open");
    store.threadSelected(null);
    store.replySet(null);
    assert.equal(calls, 0);
  });

  it("le azioni notificano quando lo stato cambia", () => {
    const store = createHubStore();
    let calls = 0;
    store.subscribe(() => (calls += 1));
    store.settingsLoaded(settingsView());
    store.threadsLoaded("general", [summary("a")]);
    store.threadSelected("a");
    store.threadLoaded(thread("a"));
    assert.equal(calls, 4);
    assert.equal(store.getState().thread.id, "a");
  });
});

describe("hub store: thread eliminato", () => {
  const listed = threadsLoaded({ ...INITIAL_HUB, enabled: true }, "general", [summary("a"), summary("b")]);

  it("il thread aperto sparisce dall'elenco e la conversazione si chiude", () => {
    const open = replySet(threadLoaded(threadSelected(listed, "a"), thread("a", [message("m1")])), "m1");
    const next = threadRemoved(open, "general", "a");
    assert.deepEqual(ids(next.threads.general), ["b"]);
    assert.equal(next.threadId, null);
    assert.equal(next.thread, null);
    assert.equal(next.replyTo, null);
  });

  it("un thread non aperto sparisce dall'elenco e la conversazione aperta resta", () => {
    const open = threadLoaded(threadSelected(listed, "a"), thread("a"));
    const next = threadRemoved(open, "general", "b");
    assert.deepEqual(ids(next.threads.general), ["a"]);
    assert.equal(next.threadId, "a");
    assert.equal(next.thread, open.thread);
  });

  it("thread o canale sconosciuto: stesso stato", () => {
    assert.equal(threadRemoved(listed, "general", "zzz"), listed);
    assert.equal(threadRemoved(listed, "ws-x", "a"), listed);
  });

  it("lo stesso id in un altro canale non chiude la conversazione aperta", () => {
    const open = threadLoaded(threadSelected(listed, "a"), thread("a"));
    assert.equal(threadRemoved(open, "ws-x", "a"), open);
  });

  it("non conta più tra i thread che aspettano l'utente", () => {
    const waiting = threadsLoaded(listed, "general", [summary("a", { waiting_user: true }), summary("b")]);
    const next = threadRemoved(waiting, "general", "a");
    assert.equal(next.threads.general.filter((t) => t.waiting_user).length, 0);
  });

  it("l'azione dello store notifica e toglie il thread", () => {
    const store = createHubStore(null);
    store.settingsLoaded(settingsView());
    store.threadsLoaded("general", [summary("a")]);
    let calls = 0;
    store.subscribe(() => (calls += 1));
    store.threadRemoved("general", "a");
    store.threadRemoved("general", "a");
    assert.equal(calls, 1);
    assert.deepEqual(store.getState().threads.general, []);
  });
});
