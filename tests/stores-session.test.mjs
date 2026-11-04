import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { EMPTY_LAYOUT, openTab, openTabBackground, setActivePane } from "../src/lib/layout.ts";
import { SESSION_SAVE_MAX_MS, SESSION_SAVE_MS, parseSession } from "../src/lib/session.ts";
import {
  buildSessionJson,
  createSessionSaver,
  mergeRestoredLayout,
  restoredTabs,
  sameSessionDeps,
  sessionDeps,
  toSessionTab,
} from "../src/stores/session.ts";
import { createTabsStore } from "../src/stores/tabs.ts";

const tab = (key, workspaceId = "w1", extra = {}) => ({
  key,
  workspaceId,
  command: "",
  startCommand: "",
  bot: `bot-${key}`,
  ...extra,
});
const worker = (key, workspaceId = "w1", extra = {}) => tab(key, workspaceId, { automated: true, ...extra });
const layoutOf = (...tabs) => tabs.reduce((l, t) => openTab(l, t.workspaceId, t.key), EMPTY_LAYOUT);
const parse = (json) => JSON.parse(json);

const CLAUDE = {
  agent_id: "claude-code",
  is_default: true,
  session_resume: {
    command_names: ["claude"],
    strip_flags: [{ names: ["--resume"], takes_value: true }],
    resume_template: "{command} --resume {id}",
  },
};

describe("buildSessionJson", () => {
  const tabs = [
    tab("a", "w1", { command: "claude", launchId: "builtin:claude" }),
    worker("wk"),
    tab("b", "w2", { bot: "Nova" }),
  ];
  const runtime = {
    a: { busy: false, title: "titolo-a", agent: { phase: "idle", agentId: "claude-code", agentSessionId: "s-1" } },
    wk: { busy: true, title: "titolo-wk" },
    b: { busy: false },
  };
  const layout = layoutOf(...tabs);

  it("esclude i worker dai tab salvati", () => {
    const doc = parse(buildSessionJson(tabs, runtime, layout, true));
    assert.deepEqual(
      doc.tabs.map((t) => t.tabId),
      ["a", "b"],
    );
  });

  it("scrive layout, nome del bot e dati dell'agente", () => {
    const doc = parse(buildSessionJson(tabs, runtime, layout, true));
    assert.equal(doc.version, 1);
    assert.equal(doc.activeWorkspaceId, layout.activeWorkspaceId);
    assert.equal(doc.activeView, "workspace");
    assert.deepEqual(doc.groups, layout.groups);
    assert.deepEqual(doc.tabs[0], {
      tabId: "a",
      workspaceId: "w1",
      launchId: "builtin:claude",
      command: "claude",
      title: "titolo-a",
      agentId: "claude-code",
      agentSessionId: "s-1",
      botName: "bot-a",
    });
    assert.equal(doc.tabs[1].botName, "Nova");
  });

  it("i titoli ci sono solo con withTitles", () => {
    const withTitles = parse(buildSessionJson(tabs, runtime, layout, true));
    const without = parse(buildSessionJson(tabs, runtime, layout, false));
    assert.equal(withTitles.tabs[0].title, "titolo-a");
    assert.equal("title" in without.tabs[0], false);
    assert.equal(without.tabs[0].agentSessionId, "s-1", "gli id dell'agente restano nella chiave");
  });

  it("la chiave senza titoli non cambia al cambiare di un titolo", () => {
    const retitled = { ...runtime, a: { ...runtime.a, title: "altro" } };
    assert.equal(buildSessionJson(tabs, runtime, layout, false), buildSessionJson(tabs, retitled, layout, false));
    assert.notEqual(buildSessionJson(tabs, runtime, layout, true), buildSessionJson(tabs, retitled, layout, true));
  });

  it("toSessionTab senza stato volatile", () => {
    assert.deepEqual(toSessionTab(tab("a"), undefined, true), {
      tabId: "a",
      workspaceId: "w1",
      launchId: undefined,
      command: "",
      title: undefined,
      agentId: undefined,
      agentSessionId: undefined,
      botName: "bot-a",
    });
  });

  it("toSessionTab: l'agente degli eventi vince, altrimenti quello con cui e' nato il tab", () => {
    const born = tab("a", "w1", { agentId: "born" });
    assert.equal(toSessionTab(born, undefined, true).agentId, "born");
    assert.equal(toSessionTab(born, { agent: { phase: "idle", agentId: "event" } }, true).agentId, "event");
    assert.equal(toSessionTab(born, { agent: { phase: "idle" } }, true).agentId, "born");
  });

  it("il risultato si rilegge con parseSession", () => {
    const json = buildSessionJson(tabs, runtime, layout, true);
    const data = parseSession(json, ["w1", "w2"]);
    assert.deepEqual(
      data.tabs.map((t) => t.tabId),
      ["a", "b"],
    );
    assert.deepEqual(data.groups[0].order, ["a"], "il worker non è nel gruppo ripristinato");
  });
});

describe("sessionDeps / sameSessionDeps", () => {
  const tabs = [tab("a"), tab("b"), worker("wk")];
  const layout = layoutOf(...tabs);
  const agent = (agentSessionId) => ({ phase: "idle", agentId: "claude-code", agentSessionId });
  const runtime = { a: { busy: false, title: "uno", agent: agent("s-1") }, wk: { busy: false, agent: agent("w-1") } };

  it("un cambio di titolo dà le stesse dipendenze", () => {
    const retitled = { ...runtime, a: { ...runtime.a, title: "due" } };
    assert.ok(sameSessionDeps(sessionDeps(tabs, runtime, layout), sessionDeps(tabs, retitled, layout)));
  });

  it("busy e fase dell'agente non contano", () => {
    const busy = { ...runtime, a: { ...runtime.a, busy: true, agent: { ...runtime.a.agent, phase: "running" } } };
    assert.ok(sameSessionDeps(sessionDeps(tabs, runtime, layout), sessionDeps(tabs, busy, layout)));
  });

  it("un agentSessionId diverso dà dipendenze diverse", () => {
    const resumed = { ...runtime, a: { ...runtime.a, agent: agent("s-2") } };
    assert.ok(!sameSessionDeps(sessionDeps(tabs, runtime, layout), sessionDeps(tabs, resumed, layout)));
  });

  it("anche l'agentId e la comparsa di un agente contano", () => {
    const other = { ...runtime, a: { ...runtime.a, agent: { ...runtime.a.agent, agentId: "altro" } } };
    assert.ok(!sameSessionDeps(sessionDeps(tabs, runtime, layout), sessionDeps(tabs, other, layout)));
    const fresh = { ...runtime, b: { busy: false, agent: agent("s-9") } };
    assert.ok(!sameSessionDeps(sessionDeps(tabs, runtime, layout), sessionDeps(tabs, fresh, layout)));
  });

  it("l'agente di un worker non conta: non si salva", () => {
    const changed = { ...runtime, wk: { busy: false, agent: agent("w-2") } };
    assert.ok(sameSessionDeps(sessionDeps(tabs, runtime, layout), sessionDeps(tabs, changed, layout)));
  });

  it("cambiano con un nuovo riferimento a tab o layout", () => {
    const base = sessionDeps(tabs, runtime, layout);
    assert.ok(!sameSessionDeps(base, sessionDeps([...tabs], runtime, layout)));
    assert.ok(!sameSessionDeps(base, sessionDeps(tabs, runtime, { ...layout })));
  });
});

describe("restoredTabs", () => {
  const POOL = ["Bolt", "Nova", "Zed", "Ada"];
  const first = () => 0;
  const savedTab = (tabId, extra = {}) => ({ tabId, workspaceId: "w1", command: "", ...extra });
  const dataOf = (tabs, groups) => ({
    version: 1,
    activeWorkspaceId: "w1",
    activeView: "workspace",
    groups: groups ?? [
      { workspaceId: "w1", order: tabs.map((t) => t.tabId), activeTabId: tabs[0].tabId, maximizedTabId: null },
    ],
    global: { order: [], activeTabId: null, maximizedTabId: null },
    tabs,
  });

  it("un tab con un nome diventato riservato prende un altro nome", () => {
    const data = dataOf([savedTab("t1", { botName: "Nova" }), savedTab("t2", { botName: "Zed" })]);
    const { tabs } = restoredTabs(data, [], POOL, [], first, ["nova"]);
    assert.notEqual(tabs[0].bot.toLowerCase(), "nova");
    assert.equal(tabs[1].bot, "Zed");
  });

  it("riporta chiave, avvio, comando di ripresa e marca il tab come ripristinato", () => {
    const data = dataOf([
      savedTab("t1", {
        command: "claude",
        launchId: "l1",
        agentId: "claude-code",
        agentSessionId: "s-1",
        title: "Lavoro",
        botName: "Nova",
      }),
      savedTab("t2"),
    ]);
    const { tabs, runtime } = restoredTabs(data, [], POOL, [CLAUDE], first);
    assert.equal(tabs[0].key, "t1");
    assert.equal(tabs[0].workspaceId, "w1");
    assert.equal(tabs[0].launchId, "l1");
    assert.equal(tabs[0].command, "claude");
    assert.equal(tabs[0].startCommand, "claude --resume s-1");
    assert.equal(tabs[0].bot, "Nova");
    assert.equal(tabs[0].restored, true);
    assert.equal(tabs[1].startCommand, "");
    assert.deepEqual(runtime.t1, {
      title: "Lavoro",
      agent: { phase: "idle", agentSessionId: "s-1", agentId: "claude-code" },
    });
    assert.deepEqual(runtime.t2, {});
  });

  it("un tab di un'integrazione non piu' abilitata riparte come shell, senza riprendere la sessione", () => {
    const data = dataOf([
      savedTab("t1", {
        command: "other --flag",
        launchId: "builtin:other",
        agentId: "other",
        agentSessionId: "s-9",
        title: "Lavoro",
      }),
      savedTab("t2", { command: "claude", agentId: "claude-code", agentSessionId: "s-1" }),
      savedTab("t3", { command: "other", agentId: "other" }),
      savedTab("t4", { command: "ls", launchId: "l1" }),
    ]);
    const { tabs, runtime, unavailableAgents } = restoredTabs(data, [], POOL, [CLAUDE], first);
    assert.deepEqual(unavailableAgents, ["other"]);
    for (const shell of [tabs[0], tabs[2]]) {
      assert.equal(shell.command, "");
      assert.equal(shell.startCommand, "");
      assert.equal(shell.launchId, undefined);
      assert.equal(shell.agentId, undefined);
      assert.equal(shell.restored, true);
    }
    assert.deepEqual(runtime.t1, { title: "Lavoro" });
    // Gli altri tab non cambiano.
    assert.equal(tabs[1].startCommand, "claude --resume s-1");
    assert.equal(tabs[1].agentId, "claude-code");
    assert.deepEqual([tabs[3].command, tabs[3].launchId], ["ls", "l1"]);
  });

  it("senza profili (non caricati) i tab si ripristinano com'erano", () => {
    const data = dataOf([savedTab("t1", { command: "other", agentId: "other" })]);
    const { tabs, unavailableAgents } = restoredTabs(data, [], POOL, [], first);
    assert.deepEqual(unavailableAgents, []);
    assert.equal(tabs[0].command, "other");
    assert.equal(tabs[0].agentId, "other");
  });

  it("nessun tab di integrazioni non abilitate: nessun avviso", () => {
    const { unavailableAgents } = restoredTabs(dataOf([savedTab("t1")]), [], POOL, [CLAUDE], first);
    assert.deepEqual(unavailableAgents, []);
  });

  it("nomi dei bot unici: il secondo con lo stesso nome e chi non ne ha ne ricevono uno nuovo", () => {
    const data = dataOf([savedTab("t1", { botName: "Bolt" }), savedTab("t2", { botName: "Bolt" }), savedTab("t3")]);
    const { tabs } = restoredTabs(data, [], POOL, [], first);
    assert.deepEqual(
      tabs.map((t) => t.bot),
      ["Bolt", "Nova", "Zed"],
    );
  });

  it("un worker arrivato prima del ripristino tiene il suo nome; un ripristinato in conflitto ne riceve uno nuovo", () => {
    const present = [worker("wk", "w1", { bot: "Bolt" })];
    const data = dataOf([savedTab("t1", { botName: "Bolt" }), savedTab("t2", { botName: "Nova" })]);
    const { tabs } = restoredTabs(data, present, POOL, [], first);
    assert.equal(tabs[0].bot, "Zed", "Bolt è del worker");
    assert.equal(tabs[1].bot, "Nova");
  });

  it("senza conflitti i nomi salvati restano, anche con tab già presenti", () => {
    const present = [tab("p", "w1", { bot: "Ada" })];
    const { tabs } = restoredTabs(dataOf([savedTab("t1", { botName: "Bolt" })]), present, POOL, [], first);
    assert.equal(tabs[0].bot, "Bolt");
  });

  it("ripristino + merge: il worker già presente sopravvive, in testa e con il suo nome", () => {
    const store = createTabsStore(first);
    const wk = store.open(worker("wk", "w1", { cwd: "C:\\work" }), POOL);
    const data = dataOf([savedTab("t1", { botName: wk.bot }), savedTab("t2")]);
    const restored = restoredTabs(data, store.getState(), POOL, [], first);
    store.merge(restored.tabs);
    assert.deepEqual(
      store.getState().map((t) => t.key),
      ["wk", "t1", "t2"],
    );
    assert.equal(store.get("wk"), wk);
    assert.equal(store.get("wk").bot, wk.bot);
    assert.notEqual(store.get("t1").bot, wk.bot);
  });
});

describe("mergeRestoredLayout", () => {
  const restored = () => setActivePane(layoutOf(tab("t1"), tab("t2"), tab("t3", "w2")), "t1");

  it("senza gruppi attuali ritorna il layout ripristinato", () => {
    const r = restored();
    assert.equal(mergeRestoredLayout(r, EMPTY_LAYOUT), r);
  });

  it("il worker già presente resta nel suo gruppo, in coda e in secondo piano", () => {
    const current = openTab(EMPTY_LAYOUT, "w1", "wk");
    const merged = mergeRestoredLayout(restored(), current);
    const w1 = merged.groups.find((g) => g.workspaceId === "w1");
    assert.deepEqual(w1.order, ["t1", "t2", "wk"]);
    assert.equal(w1.activeTabId, "t1", "la selezione ripristinata non cambia");
    assert.equal(merged.activeWorkspaceId, restored().activeWorkspaceId);
    assert.equal(merged.activeView, "workspace");
  });

  it("un worker di un workspace senza gruppo ripristinato ne crea uno in coda, senza prendere la selezione", () => {
    const current = openTabBackground(EMPTY_LAYOUT, "w9", "wk");
    const r = restored();
    const merged = mergeRestoredLayout(r, current);
    assert.deepEqual(
      merged.groups.map((g) => g.workspaceId),
      ["w1", "w2", "w9"],
    );
    assert.equal(merged.activeWorkspaceId, r.activeWorkspaceId);
  });

  it("un tab già nel layout ripristinato non si duplica e non si tocca la vista globale", () => {
    const r = restored();
    const merged = mergeRestoredLayout(r, openTab(EMPTY_LAYOUT, "w1", "t1"));
    assert.equal(merged, r);
  });
});

/** Orologio finto: `advance` esegue i timer in ordine di scadenza. */
function fakeClock() {
  let now = 0;
  let nextId = 0;
  const timers = new Map();
  return {
    now: () => now,
    setTimeout(fn, ms) {
      const id = ++nextId;
      timers.set(id, { at: now + ms, fn });
      return id;
    },
    clearTimeout(id) {
      timers.delete(id);
    },
    advance(ms) {
      const end = now + ms;
      for (;;) {
        const due = [...timers.entries()].filter(([, t]) => t.at <= end).sort((a, b) => a[1].at - b[1].at)[0];
        if (!due) break;
        timers.delete(due[0]);
        now = due[1].at;
        due[1].fn();
      }
      now = end;
    },
    pending: () => timers.size,
  };
}

const settle = () => new Promise((resolve) => setImmediate(resolve));

/** Salvatore con contenuto e titolo modificabili dal test. */
function setup(overrides = {}) {
  const clock = fakeClock();
  const doc = { content: "v1", title: "t1" };
  const saved = [];
  const errors = [];
  const saver = createSessionSaver({
    build: (withTitles) => (withTitles ? `${doc.content}|${doc.title}` : doc.content),
    save: async (json) => void saved.push(json),
    onError: (e) => errors.push(e),
    clock,
    ...overrides,
  });
  return { clock, doc, saved, errors, saver };
}

describe("createSessionSaver", () => {
  it("resta spento finché non si chiama enable", async () => {
    const { clock, saver, saved } = setup();
    assert.equal(saver.enabled, false);
    saver.changed();
    assert.equal(clock.pending(), 0);
    await saver.flush();
    clock.advance(10_000);
    assert.deepEqual(saved, []);
    saver.enable();
    assert.equal(saver.enabled, true);
  });

  it("debounce: salva una volta, dopo SESSION_SAVE_MS, con i titoli", async () => {
    const { clock, saver, saved } = setup();
    saver.enable();
    saver.changed();
    clock.advance(SESSION_SAVE_MS - 1);
    assert.deepEqual(saved, []);
    clock.advance(1);
    await settle();
    assert.deepEqual(saved, ["v1|t1"]);
    assert.equal(clock.pending(), 0);
  });

  it("modifiche ravvicinate si fondono in un solo salvataggio", async () => {
    const { clock, doc, saver, saved } = setup();
    saver.enable();
    saver.changed();
    clock.advance(300);
    doc.content = "v2";
    saver.changed();
    clock.advance(300);
    assert.deepEqual(saved, []);
    clock.advance(200);
    await settle();
    assert.deepEqual(saved, ["v2|t1"]);
  });

  it("attesa massima: modifiche continue non rimandano oltre SESSION_SAVE_MAX_MS", () => {
    const { clock, doc, saver, saved } = setup();
    saver.enable();
    const everyMs = 300; // meno di SESSION_SAVE_MS: il debounce da solo non scatterebbe mai
    for (let t = 0; t < SESSION_SAVE_MAX_MS; t += everyMs) {
      doc.content = `v${t}`;
      saver.changed();
      clock.advance(everyMs);
      if (clock.now() < SESSION_SAVE_MAX_MS) assert.deepEqual(saved, [], `nulla prima del tetto (t=${clock.now()})`);
    }
    assert.deepEqual(saved, ["v1800|t1"], "salva al tetto con l'ultimo contenuto");
  });

  it("dopo un salvataggio l'attesa massima riparte dalla modifica successiva", async () => {
    const { clock, doc, saver, saved } = setup();
    saver.enable();
    saver.changed();
    clock.advance(SESSION_SAVE_MS);
    await settle();
    clock.advance(5000);
    doc.content = "v2";
    saver.changed();
    clock.advance(SESSION_SAVE_MS);
    await settle();
    assert.deepEqual(saved, ["v1|t1", "v2|t1"]);
  });

  it("non salva se la chiave è uguale all'ultima salvata", async () => {
    const { clock, saver, saved } = setup();
    saver.enable();
    saver.changed();
    clock.advance(SESSION_SAVE_MS);
    await settle();
    saver.changed();
    assert.equal(clock.pending(), 0);
    clock.advance(10_000);
    assert.equal(saved.length, 1);
  });

  it("un cambio di solo titolo non salva", async () => {
    const { clock, doc, saver, saved } = setup();
    saver.enable();
    saver.changed();
    clock.advance(SESSION_SAVE_MS);
    await settle();
    doc.title = "t2";
    saver.changed();
    clock.advance(10_000);
    assert.equal(saved.length, 1);
  });

  it("tornare al contenuto già salvato annulla il salvataggio in attesa", async () => {
    const { clock, doc, saver, saved } = setup();
    saver.enable();
    saver.changed();
    clock.advance(SESSION_SAVE_MS);
    await settle();
    doc.content = "v2";
    saver.changed();
    assert.equal(clock.pending(), 1);
    doc.content = "v1";
    saver.changed();
    assert.equal(clock.pending(), 0);
    clock.advance(10_000);
    assert.equal(saved.length, 1);
  });

  it("errore: onError, chiave salvata azzerata, il prossimo cambio riprova", async () => {
    const boom = new Error("disco pieno");
    let fail = true;
    const { clock, saver, errors, saved } = setup({
      save: async (json) => {
        if (fail) throw boom;
        saved.push(json);
      },
    });
    saver.enable();
    saver.changed();
    clock.advance(SESSION_SAVE_MS);
    await settle();
    assert.deepEqual(errors, [boom]);
    fail = false;
    saver.changed();
    clock.advance(SESSION_SAVE_MS);
    await settle();
    assert.deepEqual(saved, ["v1|t1"], "stesso contenuto, riscritto perché il salvataggio era fallito");
    assert.equal(errors.length, 1);
  });

  it("anche un errore sincrono di save va a onError", async () => {
    const boom = new Error("sincrono");
    const { clock, saver, errors } = setup({
      save: () => {
        throw boom;
      },
    });
    saver.enable();
    saver.changed();
    clock.advance(SESSION_SAVE_MS);
    await settle();
    assert.deepEqual(errors, [boom]);
  });

  it("flush salva subito con i titoli, annulla l'attesa e non salva due volte", async () => {
    const { clock, doc, saver, saved } = setup();
    saver.enable();
    doc.title = "ultimo";
    saver.changed();
    assert.equal(clock.pending(), 1);
    await saver.flush();
    assert.deepEqual(saved, ["v1|ultimo"]);
    assert.equal(clock.pending(), 0);
    clock.advance(10_000);
    assert.equal(saved.length, 1);
  });

  it("flush rifiuta se il salvataggio fallisce, senza passare da onError", async () => {
    const boom = new Error("niente scrittura");
    const { saver, errors } = setup({
      save: async () => {
        throw boom;
      },
    });
    saver.enable();
    await assert.rejects(saver.flush(), boom);
    assert.deepEqual(errors, []);
  });

  it("dispose annulla il salvataggio in attesa", () => {
    const { clock, saver, saved } = setup();
    saver.enable();
    saver.changed();
    saver.dispose();
    assert.equal(clock.pending(), 0);
    clock.advance(10_000);
    assert.deepEqual(saved, []);
  });

  it("rispetta waitMs e maxWaitMs personalizzati", async () => {
    const { clock, saver, saved } = setup({ waitMs: 50, maxWaitMs: 80 });
    saver.enable();
    saver.changed();
    clock.advance(50);
    await settle();
    assert.equal(saved.length, 1);
  });
});
