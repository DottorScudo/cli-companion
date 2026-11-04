import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  GENERAL_CHANNEL,
  MESSAGE_MAX_CHARS,
  THREAD_TITLE_MAX_CHARS,
  appendMessage,
  authorLabel,
  channelWaiting,
  checkBody,
  checkTitle,
  excerpt,
  filterThreads,
  hubChannels,
  replyTarget,
  resolveChannel,
  sortThreads,
  summarizeThread,
  threadNeedsReload,
  upsertSummary,
} from "../src/lib/hub.ts";
import { eventLook } from "../src/lib/hubEvents.ts";

const summary = (id, extra = {}) => ({
  id,
  channel: GENERAL_CHANNEL,
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
  body: `testo ${id}`,
  reply_to: null,
  event: null,
  created_at: 1,
  ...extra,
});
const thread = (messages, extra = {}) => ({
  id: "t1",
  channel: GENERAL_CHANNEL,
  title: "T",
  state: "open",
  task_ids: [],
  proposal_id: null,
  created_at: 1,
  updated_at: 10,
  messages,
  ...extra,
});
const ids = (list) => list.map((t) => t.id);

describe("hub: canali", () => {
  it("Generale in cima, poi i workspace nell'ordine ricevuto", () => {
    const channels = hubChannels([
      { id: "b", name: "Beta" },
      { id: "a", name: "Alfa" },
    ]);
    assert.deepEqual(channels, [
      { id: "general", name: "Generale" },
      { id: "b", name: "Beta" },
      { id: "a", name: "Alfa" },
    ]);
  });

  it("senza workspace resta il Generale", () => {
    assert.deepEqual(ids(hubChannels([])), ["general"]);
  });

  it("resolveChannel ripiega sul Generale se il canale non esiste più", () => {
    const channels = hubChannels([{ id: "a", name: "Alfa" }]);
    assert.equal(resolveChannel("a", channels), "a");
    assert.equal(resolveChannel("rimosso", channels), GENERAL_CHANNEL);
  });

  it("channelWaiting: almeno un thread che aspetta; elenco non letto = no", () => {
    assert.equal(channelWaiting(undefined), false);
    assert.equal(channelWaiting([]), false);
    assert.equal(channelWaiting([summary("a"), summary("b", { waiting_user: true })]), true);
  });
});

describe("hub: elenco dei thread", () => {
  const list = [summary("a"), summary("b", { state: "closed" })];

  it("filterThreads: aperti o tutti", () => {
    assert.deepEqual(ids(filterThreads(list, "open")), ["a"]);
    assert.deepEqual(ids(filterThreads(list, "all")), ["a", "b"]);
    assert.deepEqual(filterThreads([], "open"), []);
  });

  it("sortThreads: dal più recente, a parità per creazione", () => {
    const sorted = sortThreads([
      summary("vecchio", { updated_at: 1 }),
      summary("nuovo", { updated_at: 9 }),
      summary("pari-vecchio", { updated_at: 5, created_at: 1 }),
      summary("pari-nuovo", { updated_at: 5, created_at: 3 }),
    ]);
    assert.deepEqual(ids(sorted), ["nuovo", "pari-nuovo", "pari-vecchio", "vecchio"]);
  });

  it("upsertSummary: inserisce, sostituisce e riordina", () => {
    const base = [summary("a", { updated_at: 5 }), summary("b", { updated_at: 3 })];
    assert.deepEqual(ids(upsertSummary(base, summary("c", { updated_at: 9 }))), ["c", "a", "b"]);
    const bumped = upsertSummary(base, summary("b", { updated_at: 8, title: "Nuovo" }));
    assert.deepEqual(ids(bumped), ["b", "a"]);
    assert.equal(bumped[0].title, "Nuovo");
  });

  it("upsertSummary: una versione più vecchia di quella nota non la sostituisce", () => {
    const base = [summary("a", { updated_at: 5, title: "Attuale" })];
    assert.equal(upsertSummary(base, summary("a", { updated_at: 4, title: "Vecchio" })), base);
  });
});

describe("hub: conversazione", () => {
  it("threadNeedsReload: solo per il thread aperto e solo se è cambiato", () => {
    const open = thread([message("m1")]);
    assert.equal(threadNeedsReload(null, summary("t1")), false);
    assert.equal(threadNeedsReload(open, summary("altro", { message_count: 5 })), false);
    assert.equal(threadNeedsReload(open, summary("t1", { message_count: 1, updated_at: 10 })), false);
    assert.equal(threadNeedsReload(open, summary("t1", { message_count: 2, updated_at: 10 })), true);
    assert.equal(threadNeedsReload(open, summary("t1", { message_count: 1, updated_at: 11 })), true);
  });

  it("replyTarget: trova il messaggio citato, assente se potato o senza risposta", () => {
    const t = thread([message("m1"), message("m2", { reply_to: "m1" })]);
    assert.equal(replyTarget(t, "m1")?.id, "m1");
    assert.equal(replyTarget(t, "potato"), null);
    assert.equal(replyTarget(t, null), null);
  });

  it("appendMessage: aggiunge, aggiorna l'orario, ignora i duplicati", () => {
    const t = thread([message("m1")]);
    const next = appendMessage(t, message("m2", { created_at: 50 }));
    assert.deepEqual(
      next.messages.map((m) => m.id),
      ["m1", "m2"],
    );
    assert.equal(next.updated_at, 50);
    assert.equal(appendMessage(next, message("m2")), next);
    assert.equal(appendMessage(t, message("vecchio", { created_at: 2 })).updated_at, 10);
  });

  it("summarizeThread: la riga dell'elenco di un thread appena creato", () => {
    const s = summarizeThread(thread([message("m1")], { task_ids: ["x", "y"] }));
    assert.deepEqual(s, {
      id: "t1",
      channel: GENERAL_CHANNEL,
      title: "T",
      state: "open",
      task_count: 2,
      message_count: 1,
      waiting_user: false,
      created_at: 1,
      updated_at: 10,
    });
  });
});

describe("hub: testi", () => {
  it("checkBody: senza spazi ai bordi, non vuoto", () => {
    assert.deepEqual(checkBody("  ciao\n"), { ok: true, body: "ciao" });
    assert.equal(checkBody("").ok, false);
    assert.equal(checkBody(" \n\t ").ok, false);
  });

  it("checkBody: 8000 caratteri sì, 8001 no (caratteri, non unità UTF-16)", () => {
    assert.equal(checkBody("a".repeat(MESSAGE_MAX_CHARS)).ok, true);
    assert.equal(checkBody("a".repeat(MESSAGE_MAX_CHARS + 1)).ok, false);
    assert.equal(checkBody("😀".repeat(MESSAGE_MAX_CHARS)).ok, true);
    assert.equal(checkBody("😀".repeat(MESSAGE_MAX_CHARS + 1)).ok, false);
  });

  it("checkTitle: non vuoto e nel limite", () => {
    assert.deepEqual(checkTitle(" Idea "), { ok: true, body: "Idea" });
    assert.equal(checkTitle("   ").ok, false);
    assert.equal(checkTitle("a".repeat(THREAD_TITLE_MAX_CHARS)).ok, true);
    assert.equal(checkTitle("a".repeat(THREAD_TITLE_MAX_CHARS + 1)).ok, false);
  });

  it("excerpt: una riga, troncata con i puntini", () => {
    assert.equal(excerpt("  uno\n due\t tre "), "uno due tre");
    assert.equal(excerpt("abcdef", 4), "abc…");
    assert.equal(excerpt("abcd", 4), "abcd");
    assert.equal(excerpt("😀😀😀😀", 3), "😀😀…");
  });

  it("authorLabel: utente, sistema e bot", () => {
    assert.equal(authorLabel({ type: "user" }), "Tu");
    assert.equal(authorLabel({ type: "system" }), "Sistema");
    assert.equal(authorLabel({ type: "bot", name: "Mario" }), "Mario");
  });
});

describe("hubEvents: eventLook", () => {
  it("ogni tipo noto ha etichetta, icona e tono", () => {
    const kinds = {
      proposal_created: ["Proposta", "proposal", "warning"],
      proposal_accepted: ["Proposta accettata", "proposal", "success"],
      proposal_rejected: ["Proposta rifiutata", "proposal", "neutral"],
      task_created: ["Creato", "generic", "neutral"],
      task_started: ["Avviato", "started", "neutral"],
      task_done: ["Completato", "done", "success"],
      task_failed: ["Fallito", "failed", "danger"],
      task_attention: ["Attenzione", "attention", "warning"],
      task_deleted: ["Eliminato", "deleted", "neutral"],
    };
    for (const [kind, [label, icon, tone]] of Object.entries(kinds)) {
      assert.deepEqual(eventLook(kind), { label, icon, tone }, kind);
    }
  });

  it("un tipo sconosciuto ha la resa generica", () => {
    assert.deepEqual(eventLook("tipo_futuro"), { label: "Evento", icon: "generic", tone: "neutral" });
  });
});
