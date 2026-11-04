import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { EMPTY_LAYOUT, GLOBAL_ID, openTab, pinTab, setActivePane } from "../src/lib/layout.ts";
import { GROUP_STATUS } from "../src/lib/agents.ts";
import {
  attentionCount,
  sameSidebarTerminals,
  sameTitleTabs,
  sidebarTerminals,
  tabLabel,
  titleBarTabs,
} from "../src/stores/selectors.ts";

const BUILTINS = [
  { id: "builtin:claude", name: "Claude Code", command: "claude", restore_command: "claude", agent_id: "claude-code" },
];
const IMAGE = { png: "AAAA", color: "#112233" };
const BOTS = { enabled: true, names: ["Bolt"], images: { Bolt: IMAGE } };
const imageOf = (name) => (name === "Bolt" ? IMAGE : undefined);

const ws = (id, extra = {}) => ({
  id,
  name: `WS ${id}`,
  launches: [{ id: "dev", name: "Dev server", command: "npm run dev" }],
  ...extra,
});
const WORKSPACES = [ws("w1"), ws("w2")];
const tab = (key, workspaceId = "w1", extra = {}) => ({
  key,
  workspaceId,
  command: "",
  startCommand: "",
  bot: "Bolt",
  ...extra,
});

/** w1: a, b, c — w2: d. */
const TABS = [tab("a"), tab("b"), tab("c"), tab("d", "w2")];
const layout = () => TABS.reduce((l, t) => openTab(l, t.workspaceId, t.key), EMPTY_LAYOUT);

describe("tabLabel", () => {
  const w = WORKSPACES[0];

  it("priorità: titolo del task > titolo OSC > nome dell'avvio > nome del workspace", () => {
    const t = tab("a", "w1", { launchId: "dev" });
    assert.equal(
      tabLabel(
        t,
        { busy: false, title: "vim", automation: { title: "Task X", status: "running", agentId: "x" } },
        w,
        BUILTINS,
      ),
      "Task X",
    );
    assert.equal(tabLabel(t, { busy: false, title: "vim" }, w, BUILTINS), "vim");
    assert.equal(tabLabel(t, { busy: false }, w, BUILTINS), "Dev server");
    assert.equal(tabLabel(tab("a"), { busy: false }, w, BUILTINS), "WS w1");
    assert.equal(tabLabel(tab("a"), undefined, w, BUILTINS), "WS w1");
  });

  it("il nome dell'avvio viene anche dagli integrati", () => {
    assert.equal(tabLabel(tab("a", "w1", { launchId: "builtin:claude" }), undefined, w, BUILTINS), "Claude Code");
  });

  it("avvio sconosciuto: ripiega sul workspace", () => {
    assert.equal(tabLabel(tab("a", "w1", { launchId: "sparito" }), undefined, w, BUILTINS), "WS w1");
  });

  it("workspace mancante: stringa vuota, ma il titolo OSC vince", () => {
    assert.equal(tabLabel(tab("a", "gone", { launchId: "dev" }), undefined, undefined, BUILTINS), "");
    assert.equal(tabLabel(tab("a"), { busy: false, title: "vim" }, undefined, BUILTINS), "vim");
  });

  it("un titolo del task vuoto non conta", () => {
    const rt = { busy: false, automation: { title: "", status: "running", agentId: "x" } };
    assert.equal(tabLabel(tab("a"), rt, WORKSPACES[0], BUILTINS), "WS w1");
  });
});

describe("titleBarTabs", () => {
  it("senza tab fissati non c'è la scheda Globale; un gruppo per workspace con nome e conteggio", () => {
    const { globalTab, groups } = titleBarTabs(layout(), TABS, {}, WORKSPACES);
    assert.equal(globalTab, null);
    assert.deepEqual(
      groups.map((g) => [g.id, g.name, g.count, g.status]),
      [
        ["w1", "WS w1", 3, null],
        ["w2", "WS w2", 1, null],
      ],
    );
  });

  it("con tab fissati compare Globale, con il conteggio dei fissati", () => {
    const l = pinTab(pinTab(layout(), "a"), "d");
    const { globalTab } = titleBarTabs(l, TABS, {}, WORKSPACES);
    assert.equal(globalTab.id, GLOBAL_ID);
    assert.equal(globalTab.name, "Globale");
    assert.equal(globalTab.count, 2);
  });

  it("nome del gruppo: dall'elenco dei workspace, altrimenti l'id", () => {
    const { groups } = titleBarTabs(layout(), TABS, {}, [ws("w1", { name: "Rinominato" })]);
    assert.deepEqual(
      groups.map((g) => g.name),
      ["Rinominato", "w2"],
    );
  });

  it("stato del gruppo: attenzione > fatto > in corso > nessuno", () => {
    const runtime = {
      a: { busy: true },
      b: { busy: false, agent: { phase: "needs-input" } },
      c: { busy: false, agent: { phase: "finished" } },
      d: { busy: true },
    };
    const { groups } = titleBarTabs(layout(), TABS, runtime, WORKSPACES);
    assert.equal(groups[0].status, GROUP_STATUS.alert);
    assert.equal(groups[1].status, GROUP_STATUS.busy);
    const done = titleBarTabs(layout(), TABS, { c: runtime.c }, WORKSPACES).groups[0].status;
    assert.equal(done, GROUP_STATUS.check);
  });

  it("la scheda Globale aggrega solo lo stato dei tab fissati", () => {
    const l = pinTab(layout(), "d");
    const runtime = { a: { busy: false, agent: { phase: "needs-input" } }, d: { busy: true } };
    assert.equal(titleBarTabs(l, TABS, runtime, WORKSPACES).globalTab.status, GROUP_STATUS.busy);
  });

  it("un tab nel layout ma non più tra i tab non conta per lo stato", () => {
    const runtime = { b: { busy: false, agent: { phase: "needs-input" } } };
    const withoutB = TABS.filter((t) => t.key !== "b");
    assert.equal(titleBarTabs(layout(), withoutB, runtime, WORKSPACES).groups[0].status, null);
  });
});

describe("sameTitleTabs", () => {
  const derive = (runtime = {}, l = layout()) => titleBarTabs(l, TABS, runtime, WORKSPACES);

  it("ricalcolare con gli stessi dati dà tab uguali (stati nuovi ma uguali per valore)", () => {
    const a = derive({ a: { busy: true } });
    const b = derive({ a: { busy: true } });
    assert.notEqual(a.groups[0].status, null);
    assert.ok(sameTitleTabs(a, b));
  });

  it("cambia se cambia stato, conteggio, nome o presenza di Globale", () => {
    const base = derive();
    assert.ok(!sameTitleTabs(base, derive({ a: { busy: true } })));
    assert.ok(!sameTitleTabs(base, titleBarTabs(layout(), TABS, {}, [ws("w1", { name: "Altro" }), ws("w2")])));
    assert.ok(!sameTitleTabs(base, derive({}, pinTab(layout(), "a"))));
    assert.ok(!sameTitleTabs(derive({}, pinTab(layout(), "a")), base));
    assert.ok(!sameTitleTabs(base, { ...base, groups: base.groups.slice(1) }));
  });

  it("due Globale uguali si confrontano per valore", () => {
    const l = pinTab(layout(), "a");
    assert.ok(sameTitleTabs(derive({}, l), derive({}, l)));
    assert.ok(!sameTitleTabs(derive({}, l), derive({ a: { busy: true } }, l)));
  });
});

describe("sidebarTerminals", () => {
  const derive = (workspaceId, over = {}) =>
    sidebarTerminals(
      workspaceId,
      over.layout ?? layout(),
      over.tabs ?? TABS,
      over.runtime ?? {},
      over.workspaces ?? WORKSPACES,
      BUILTINS,
      over.bots ?? BOTS,
      imageOf,
    );

  it("terminali del workspace nell'ordine della griglia, con etichetta e stato", () => {
    const list = derive("w1", { runtime: { b: { busy: true, title: "vim" } } });
    assert.deepEqual(
      list.map((t) => [t.key, t.label, t.status.tone]),
      [
        ["a", "WS w1", "idle"],
        ["b", "vim", "busy"],
        ["c", "WS w1", "idle"],
      ],
    );
  });

  it("segue l'ordine del layout, non quello di creazione", () => {
    const l = {
      ...layout(),
      groups: layout().groups.map((g) => (g.workspaceId === "w1" ? { ...g, order: ["c", "a", "b"] } : g)),
    };
    assert.deepEqual(
      derive("w1", { layout: l }).map((t) => t.key),
      ["c", "a", "b"],
    );
  });

  it("attivo e fissato", () => {
    const l = pinTab(setActivePane(layout(), "b"), "c");
    const byKey = Object.fromEntries(derive("w1", { layout: l }).map((t) => [t.key, t]));
    assert.deepEqual([byKey.a.active, byKey.b.active, byKey.c.active], [false, true, false]);
    assert.deepEqual([byKey.a.pinned, byKey.b.pinned, byKey.c.pinned], [false, false, true]);
  });

  it("bot e immagine solo con la funzione attiva", () => {
    const [on] = derive("w2");
    assert.equal(on.bot, "Bolt");
    assert.equal(on.botImage, IMAGE);
    const [off] = derive("w2", { bots: { ...BOTS, enabled: false } });
    assert.equal(off.bot, undefined);
    assert.equal(off.botImage, undefined);
    assert.equal(derive("w2", { tabs: [tab("d", "w2", { bot: "Nova" })] })[0].botImage, undefined);
  });

  it("workspace senza gruppo: elenco vuoto; tab sparito dai tab: saltato", () => {
    assert.deepEqual(derive("w9"), []);
    assert.deepEqual(
      derive("w1", { tabs: TABS.filter((t) => t.key !== "b") }).map((t) => t.key),
      ["a", "c"],
    );
  });

  it("l'etichetta usa il nome corrente del workspace dall'elenco", () => {
    const [first] = derive("w1", { workspaces: [ws("w1", { name: "Nuovo nome" })] });
    assert.equal(first.label, "Nuovo nome");
  });
});

describe("sameSidebarTerminals", () => {
  const list = (runtime = {}, bots = BOTS) =>
    sidebarTerminals("w1", layout(), TABS, runtime, WORKSPACES, BUILTINS, bots, imageOf);

  it("elenchi ricalcolati con gli stessi dati sono uguali", () => {
    assert.ok(sameSidebarTerminals(list({ a: { busy: true } }), list({ a: { busy: true } })));
  });

  it("cambia con stato, etichetta, bot, lunghezza", () => {
    const base = list();
    assert.ok(!sameSidebarTerminals(base, list({ a: { busy: true } })));
    assert.ok(!sameSidebarTerminals(base, list({ a: { busy: false, title: "x" } })));
    assert.ok(!sameSidebarTerminals(base, list({}, { ...BOTS, enabled: false })));
    assert.ok(!sameSidebarTerminals(base, base.slice(1)));
  });

  it("cambia con attivo e fissato", () => {
    const base = list();
    assert.ok(
      !sameSidebarTerminals(
        base,
        base.map((t, i) => (i === 0 ? { ...t, active: !t.active } : t)),
      ),
    );
    assert.ok(
      !sameSidebarTerminals(
        base,
        base.map((t, i) => (i === 0 ? { ...t, pinned: !t.pinned } : t)),
      ),
    );
  });
});

describe("attentionCount", () => {
  it("conta i tab che aspettano una risposta", () => {
    const runtime = {
      a: { busy: false, agent: { phase: "needs-input" } },
      b: { busy: false, agent: { phase: "finished" } },
      c: { busy: false, agent: { phase: "needs-input" } },
    };
    assert.equal(attentionCount(TABS, runtime), 2);
    assert.equal(attentionCount(TABS, {}), 0);
    assert.equal(attentionCount([], runtime), 0);
  });

  it("un agente terminato (session-end) non conta", () => {
    assert.equal(attentionCount(TABS, { a: { busy: false, agent: { phase: "needs-input", ended: true } } }), 0);
  });
});
