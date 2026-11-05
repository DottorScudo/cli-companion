import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  CHAT_PREVIEW_CONCURRENCY,
  CHAT_PREVIEW_LIMIT,
  chatList,
  messagePreviewText,
  previewAuthor,
  previewOf,
  previewsToRead,
  withPreview,
} from "../src/lib/hubChats.ts";
import { mapLimit } from "../src/lib/mapLimit.ts";

const channels = [
  { id: "general", name: "Generale" },
  { id: "w1", name: "Sito Web" },
  { id: "w2", name: "Backend" },
];
const summary = (id, extra = {}) => ({
  id,
  channel: "general",
  title: `Thread ${id}`,
  state: "open",
  task_count: 0,
  message_count: 1,
  waiting_user: false,
  owner: null,
  created_at: 1,
  updated_at: 1,
  ...extra,
});
const message = (extra = {}) => ({
  id: "m",
  author: { type: "user" },
  kind: "text",
  body: "ciao",
  reply_to: null,
  event: null,
  attachments: [],
  created_at: 5,
  ...extra,
});
const thread = (messages, extra = {}) => ({
  id: "t",
  channel: "general",
  title: "t",
  state: "open",
  task_ids: [],
  proposal_id: null,
  owner: null,
  created_at: 1,
  updated_at: 9,
  summary: null,
  messages,
  ...extra,
});
const file = (name) => ({ id: `att-${name}`, name, mime: "text/plain", size: 1 });
const ids = (entries) => entries.map((e) => e.thread.id);

describe("chatList", () => {
  const byChannel = {
    general: [summary("g1", { updated_at: 10 }), summary("g2", { updated_at: 30, state: "closed" })],
    w1: [summary("s1", { channel: "w1", updated_at: 20, title: "Login" })],
    w2: [summary("b1", { channel: "w2", updated_at: 20, title: "API" })],
    hidden: [summary("x1", { channel: "hidden", updated_at: 99 })],
  };

  it("unisce i canali visibili dal più recente, a parità per id, senza i canali non visibili", () => {
    assert.deepEqual(ids(chatList(byChannel, channels, "all", "")), ["g2", "b1", "s1", "g1"]);
  });

  it("ogni voce porta il nome del suo canale", () => {
    const names = Object.fromEntries(chatList(byChannel, channels, "all", "").map((e) => [e.thread.id, e.channelName]));
    assert.deepEqual(names, { g1: "Generale", g2: "Generale", s1: "Sito Web", b1: "Backend" });
  });

  it("il filtro aperti scarta i thread chiusi, come nella vista per canale", () => {
    assert.deepEqual(ids(chatList(byChannel, channels, "open", "")), ["b1", "s1", "g1"]);
  });

  it("la ricerca ignora maiuscole e spazi ai bordi e guarda titolo e workspace", () => {
    assert.deepEqual(ids(chatList(byChannel, channels, "all", "  LOGIN ")), ["s1"]);
    assert.deepEqual(ids(chatList(byChannel, channels, "all", "sito")), ["s1"]);
    assert.deepEqual(ids(chatList(byChannel, channels, "all", "generale")), ["g2", "g1"]);
    assert.deepEqual(ids(chatList(byChannel, channels, "all", "   ")), ["g2", "b1", "s1", "g1"]);
    assert.deepEqual(chatList(byChannel, channels, "all", "inesistente"), []);
  });

  it("la ricerca regge i caratteri speciali e gli unicode", () => {
    const list = { general: [summary("u", { title: "Città (v2)? [x]" })] };
    assert.deepEqual(ids(chatList(list, channels, "all", "CITTÀ (v2)? [")), ["u"]);
  });

  it("filtra per un solo canale; un canale non visibile non dà nulla", () => {
    assert.deepEqual(ids(chatList(byChannel, channels, "all", "", "w1")), ["s1"]);
    assert.deepEqual(chatList(byChannel, channels, "all", "", "hidden"), []);
  });

  it("canali non ancora letti o nessun canale: elenco vuoto", () => {
    assert.deepEqual(chatList({}, channels, "all", ""), []);
    assert.deepEqual(chatList(byChannel, [], "all", ""), []);
  });

  it("non modifica gli elenchi ricevuti", () => {
    const before = JSON.stringify(byChannel);
    chatList(byChannel, channels, "all", "");
    assert.equal(JSON.stringify(byChannel), before);
  });
});

describe("anteprima dell'ultimo messaggio", () => {
  it("autore: Tu, @segretario, Sistema", () => {
    assert.equal(previewAuthor({ type: "user" }), "Tu");
    assert.equal(previewAuthor({ type: "bot", name: "mario" }), "@mario");
    assert.equal(previewAuthor({ type: "system" }), "Sistema");
  });

  it("testo su una riga, accorciato", () => {
    assert.equal(messagePreviewText(message({ body: "una\n\n  riga   sola" })), "una riga sola");
    const long = messagePreviewText(message({ body: "a".repeat(200) }));
    assert.equal([...long].length, 80);
    assert.ok(long.endsWith("…"));
  });

  it("un evento usa il suo testo breve; senza testo, la sua etichetta", () => {
    const event = { kind: "task_done" };
    assert.equal(messagePreviewText(message({ kind: "event", body: "Task completato", event })), "Task completato");
    assert.equal(messagePreviewText(message({ kind: "event", body: "", event })), "Completato");
  });

  it("solo allegati: il nome se è uno, altrimenti quanti sono", () => {
    assert.equal(messagePreviewText(message({ body: "", attachments: [file("a.txt")] })), "📎 a.txt");
    assert.equal(messagePreviewText(message({ body: "  ", attachments: [file("a"), file("b")] })), "📎 2 allegati");
    assert.equal(messagePreviewText(message({ body: "guarda", attachments: [file("a")] })), "guarda");
  });

  it("previewOf prende l'ultimo messaggio", () => {
    const t = thread([
      message({ body: "uno" }),
      message({ author: { type: "bot", name: "bolt" }, body: "due", created_at: 7 }),
    ]);
    assert.deepEqual(previewOf(t), { updatedAt: 9, author: "@bolt", text: "due", at: 7 });
  });

  it("senza messaggi: nessuna anteprima, ma ricordata per non rileggere", () => {
    assert.deepEqual(previewOf(thread([])), { updatedAt: 9, author: "", text: "", at: 0 });
  });
});

describe("previewsToRead", () => {
  const preview = (updatedAt) => ({ updatedAt, author: "Tu", text: "x", at: 1 });
  const list = (n) => Array.from({ length: n }, (_, i) => summary(`t${i}`, { updated_at: 100 }));

  it("limiti dichiarati", () => {
    assert.equal(CHAT_PREVIEW_LIMIT, 30);
    assert.equal(CHAT_PREVIEW_CONCURRENCY, 4);
  });

  it("rilegge solo le chat senza anteprima o con updated_at cambiato", () => {
    const threads = [summary("a", { updated_at: 5 }), summary("b", { updated_at: 6 }), summary("c", { updated_at: 7 })];
    const previews = { a: preview(5), b: preview(4) };
    assert.deepEqual(
      previewsToRead(threads, previews).map((t) => t.id),
      ["b", "c"],
    );
  });

  it("un'anteprima più recente del riassunto (thread aperto già aggiornato) non si rilegge", () => {
    assert.deepEqual(previewsToRead([summary("a", { updated_at: 5 })], { a: preview(8) }), []);
  });

  it("si limita alle prime 30 dell'elenco", () => {
    assert.equal(previewsToRead(list(45), {}).length, CHAT_PREVIEW_LIMIT);
    assert.equal(previewsToRead(list(45), {})[0].id, "t0");
    assert.equal(previewsToRead(list(45), {}, 3).length, 3);
  });

  it("nessuna chat: nulla da leggere", () => {
    assert.deepEqual(previewsToRead([], {}), []);
  });
});

describe("withPreview", () => {
  const preview = (updatedAt, text = "x") => ({ updatedAt, author: "Tu", text, at: 1 });

  it("aggiunge, sostituisce con una più nuova e ignora una più vecchia", () => {
    const a = withPreview({}, "t", preview(5));
    assert.deepEqual(a, { t: preview(5) });
    assert.equal(withPreview(a, "t", preview(4)), a);
    assert.equal(withPreview(a, "t", preview(5)), a);
    assert.equal(withPreview(a, "t", preview(6)).t.updatedAt, 6);
    assert.equal(withPreview(a, "t", preview(5, "y")).t.text, "y");
  });
});

describe("mapLimit", () => {
  it("non supera il limite e fa tutto", async () => {
    let running = 0;
    let peak = 0;
    const done = [];
    await mapLimit([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 4, async (n) => {
      running += 1;
      peak = Math.max(peak, running);
      await new Promise((resolve) => setImmediate(resolve));
      running -= 1;
      done.push(n);
    });
    assert.equal(peak, 4);
    assert.equal(done.length, 10);
  });

  it("elenco vuoto o limite non valido", async () => {
    await mapLimit([], 4, async () => assert.fail("non deve partire"));
    const done = [];
    await mapLimit([1, 2], 0, async (n) => void done.push(n));
    assert.deepEqual(done, [1, 2]);
  });
});
