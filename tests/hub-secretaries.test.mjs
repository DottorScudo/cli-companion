import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  activityLabel,
  allowsChannel,
  completeMention,
  expiryText,
  filterMentions,
  findSecretary,
  isConfigured,
  mentionQuery,
  mentionable,
  minutesLeft,
  moveActive,
  promptFirstLine,
  secretaryActivity,
  secretaryNames,
  splitMentions,
  threadSession,
  threadStatuses,
  typingLabel,
  typingNames,
} from "../src/lib/hubSecretaries.ts";

const secretary = (name, extra = {}) => ({
  name,
  image: null,
  system_prompt: "You help.",
  tools: [],
  channels: { mode: "all" },
  ...extra,
});
const status = (name, extra = {}) => ({
  channel: "general",
  thread_id: "t1",
  secretary: name,
  expires_at: 10 * 60_000,
  working: false,
  ...extra,
});

describe("elenco dei segretari", () => {
  it("senza system prompt (anche solo spazi) non è configurato", () => {
    assert.equal(isConfigured(secretary("mario")), true);
    assert.equal(isConfigured(secretary("mario", { system_prompt: "" })), false);
    assert.equal(isConfigured(secretary("mario", { system_prompt: " \n " })), false);
  });

  it("nomi e ricerca per nome senza badare alle maiuscole", () => {
    const list = [secretary("mario"), secretary("luisa")];
    assert.deepEqual(secretaryNames(list), ["mario", "luisa"]);
    assert.equal(findSecretary(list, "MARIO")?.name, "mario");
    assert.equal(findSecretary(list, "giorgio"), undefined);
  });

  it("canali ammessi: tutti, o solo quelli scelti", () => {
    assert.equal(allowsChannel(secretary("a"), "ws1"), true);
    const only = secretary("a", { channels: { mode: "only", channels: ["general", "ws2"] } });
    assert.equal(allowsChannel(only, "general"), true);
    assert.equal(allowsChannel(only, "ws1"), false);
    assert.equal(allowsChannel(secretary("a", { channels: { mode: "only", channels: [] } }), "general"), false);
  });

  it("mentionable: solo i configurati ammessi nel canale", () => {
    const list = [
      secretary("tutti"),
      secretary("solo-ws1", { channels: { mode: "only", channels: ["ws1"] } }),
      secretary("vuoto", { system_prompt: "" }),
    ];
    assert.deepEqual(
      mentionable(list, "general").map((s) => s.name),
      ["tutti"],
    );
    assert.deepEqual(
      mentionable(list, "ws1").map((s) => s.name),
      ["tutti", "solo-ws1"],
    );
  });

  it("promptFirstLine: prima riga non vuota, trimmata", () => {
    assert.equal(promptFirstLine("\n  \n  You are Mario.\nSecond"), "You are Mario.");
    assert.equal(promptFirstLine("a\r\nb"), "a");
    assert.equal(promptFirstLine(""), "");
  });
});

describe("mentionQuery", () => {
  it("riconosce la menzione in corso al cursore", () => {
    assert.deepEqual(mentionQuery("@", 1), { start: 0, query: "" });
    assert.deepEqual(mentionQuery("@ma", 3), { start: 0, query: "ma" });
    assert.deepEqual(mentionQuery("ciao @ma", 8), { start: 5, query: "ma" });
    assert.deepEqual(mentionQuery("ciao\n@ma", 8), { start: 5, query: "ma" });
    assert.deepEqual(mentionQuery("(@ma", 4), { start: 1, query: "ma" });
  });

  it("usa il testo fino al cursore, non oltre", () => {
    assert.deepEqual(mentionQuery("@mario ok", 3), { start: 0, query: "ma" });
  });

  it("non è una menzione dopo uno spazio, in un indirizzo email o in @@", () => {
    assert.equal(mentionQuery("@mario ", 7), null);
    assert.equal(mentionQuery("scrivi a posta@ma", 17), null);
    assert.equal(mentionQuery("@@ma", 4), null);
    assert.equal(mentionQuery("nessuna", 7), null);
    assert.equal(mentionQuery("", 0), null);
  });
});

describe("filterMentions e completeMention", () => {
  const list = [secretary("mario"), secretary("marta"), secretary("luisa")];

  it("filtra per prefisso senza badare alle maiuscole, in ordine alfabetico", () => {
    assert.deepEqual(
      filterMentions(list, "MA").map((s) => s.name),
      ["mario", "marta"],
    );
    assert.deepEqual(
      filterMentions(list, "").map((s) => s.name),
      ["luisa", "mario", "marta"],
    );
    assert.deepEqual(filterMentions(list, "x"), []);
  });

  it("non modifica l'elenco ricevuto", () => {
    const copy = [...list];
    filterMentions(list, "");
    assert.deepEqual(list, copy);
  });

  it("completa con @nome e uno spazio, cursore dopo", () => {
    const done = completeMention("ciao @ma", { start: 5, query: "ma" }, 8, "mario");
    assert.equal(done.text, "ciao @mario ");
    assert.equal(done.caret, done.text.length);
  });

  it("al centro del testo non raddoppia lo spazio che c'è già", () => {
    const done = completeMention("@ma poi", { start: 0, query: "ma" }, 3, "mario");
    assert.equal(done.text, "@mario poi");
    assert.equal(done.caret, 7);
  });

  it("prima di un a capo aggiunge lo spazio", () => {
    const done = completeMention("@ma\nriga", { start: 0, query: "ma" }, 3, "mario");
    assert.equal(done.text, "@mario \nriga");
    assert.equal(done.caret, 7);
  });
});

describe("moveActive", () => {
  it("scorre con giro in entrambi i versi", () => {
    assert.equal(moveActive(0, 1, 3), 1);
    assert.equal(moveActive(2, 1, 3), 0);
    assert.equal(moveActive(0, -1, 3), 2);
  });

  it("menu vuoto = nessuna voce", () => {
    assert.equal(moveActive(0, 1, 0), -1);
  });
});

describe("splitMentions", () => {
  const names = ["mario", "luisa"];
  const parts = (text) => splitMentions(text, names).map((s) => [s.text, s.mention]);

  it("separa le menzioni dei segretari noti dal resto", () => {
    assert.deepEqual(parts("@mario a che punto siamo?"), [
      ["@mario", true],
      [" a che punto siamo?", false],
    ]);
    assert.deepEqual(parts("ciao @Luisa e @mario!"), [
      ["ciao ", false],
      ["@Luisa", true],
      [" e ", false],
      ["@mario", true],
      ["!", false],
    ]);
  });

  it("ignora nomi sconosciuti, email, parole più lunghe e testo senza menzioni", () => {
    assert.deepEqual(parts("@giorgio"), [["@giorgio", false]]);
    assert.deepEqual(parts("posta@mario.it"), [["posta@mario.it", false]]);
    assert.deepEqual(parts("@mariolino @mario_1 @màrio"), [["@mariolino @mario_1 @màrio", false]]);
    assert.deepEqual(parts("niente"), [["niente", false]]);
    assert.deepEqual(parts(""), []);
  });

  it("il trattino finale non fa parte della menzione", () => {
    assert.deepEqual(parts("(@mario-)"), [
      ["(", false],
      ["@mario", true],
      ["-)", false],
    ]);
  });

  it("regge unicode attorno alle menzioni", () => {
    assert.deepEqual(parts("🙂 «@mario» è pronto 🙂"), [
      ["🙂 «", false],
      ["@mario", true],
      ["» è pronto 🙂", false],
    ]);
  });

  it("senza segretari non evidenzia nulla", () => {
    assert.deepEqual(splitMentions("@mario", []), [{ text: "@mario", mention: false }]);
  });

  it("i tratti rimessi insieme ridanno il testo", () => {
    const text = "x @mario y @luisa z @nessuno";
    assert.equal(
      splitMentions(text, names)
        .map((s) => s.text)
        .join(""),
      text,
    );
  });
});

describe("stato dei segretari", () => {
  it("non configurato / dorme / sveglio, con la scadenza più lontana", () => {
    assert.deepEqual(secretaryActivity(secretary("m", { system_prompt: "" }), [status("m")]), {
      kind: "unconfigured",
    });
    assert.deepEqual(secretaryActivity(secretary("m"), []), { kind: "asleep" });
    assert.deepEqual(secretaryActivity(secretary("m"), [status("altro")]), { kind: "asleep" });
    assert.deepEqual(
      secretaryActivity(secretary("m"), [
        status("m", { thread_id: "a", expires_at: 5 }),
        status("m", { thread_id: "b", expires_at: 9, working: true }),
      ]),
      { kind: "awake", expiresAt: 9, working: true },
    );
  });

  it("minutesLeft arrotonda per eccesso, almeno 1 finché non è scaduta", () => {
    assert.equal(minutesLeft(60_000 * 12, 0), 12);
    assert.equal(minutesLeft(60_000 * 12 + 1, 0), 13);
    assert.equal(minutesLeft(1, 0), 1);
    assert.equal(minutesLeft(0, 0), 0);
    assert.equal(minutesLeft(10, 100), 0);
  });

  it("testi di scadenza e di stato", () => {
    assert.equal(expiryText(60_000 * 12, 0), "scade tra 12 min");
    assert.equal(expiryText(5, 10), "scade a momenti");
    assert.equal(activityLabel({ kind: "unconfigured" }, 0), "non configurato");
    assert.equal(activityLabel({ kind: "asleep" }, 0), "dorme");
    assert.equal(
      activityLabel({ kind: "awake", expiresAt: 60_000 * 12, working: false }, 0),
      "sveglio · scade tra 12 min",
    );
    assert.equal(activityLabel({ kind: "awake", expiresAt: 60_000 * 12, working: true }, 0), "al lavoro");
  });
});

describe("segretari sul thread", () => {
  const list = [
    status("mario", { expires_at: 100 }),
    status("luisa", { expires_at: 300, working: true }),
    status("giorgio", { thread_id: "t2", expires_at: 999, working: true }),
    status("ines", { channel: "ws1", expires_at: 999, working: true }),
  ];

  it("threadStatuses filtra per canale e thread", () => {
    assert.deepEqual(
      threadStatuses(list, "general", "t1").map((s) => s.secretary),
      ["mario", "luisa"],
    );
    assert.deepEqual(threadStatuses(list, "general", "nessuno"), []);
  });

  it("la sessione attiva è quella che scade per ultima", () => {
    assert.equal(threadSession(list, "general", "t1")?.secretary, "luisa");
    assert.equal(threadSession(list, "general", "nessuno"), null);
    assert.equal(threadSession([], "general", "t1"), null);
  });

  it("chi sta scrivendo: solo chi lavora su quel thread", () => {
    assert.deepEqual(typingNames(list, "general", "t1"), ["luisa"]);
    assert.deepEqual(typingNames(list, "ws1", "t1"), ["ines"]);
    assert.deepEqual(typingNames(list, "general", "t3"), []);
  });

  it("typingLabel", () => {
    assert.equal(typingLabel([]), "");
    assert.equal(typingLabel(["mario"]), "mario sta scrivendo…");
    assert.equal(typingLabel(["mario", "luisa"]), "mario e luisa stanno scrivendo…");
    assert.equal(typingLabel(["a", "b", "c"]), "a, b e c stanno scrivendo…");
  });
});
