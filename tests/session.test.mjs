import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  debounceDelay,
  isSafeAgentSessionId,
  isSafeTabId,
  parseSession,
  restoreCommand,
  serializeSession,
  unavailableAgentsNotice,
} from "../src/lib/session.ts";

const tab = (tabId, workspaceId, extra = {}) => ({ tabId, workspaceId, command: "", ...extra });
const doc = (o) => JSON.stringify({ version: 1, ...o });

describe("sessione: serializzazione e lettura", () => {
  it("roundtrip", () => {
    const data = {
      activeWorkspaceId: "w2",
      activeView: "global",
      global: { order: ["c", "a"], activeTabId: "a", maximizedTabId: "c" },
      groups: [
        { workspaceId: "w1", order: ["a", "b"], activeTabId: "b", maximizedTabId: "a" },
        { workspaceId: "w2", order: ["c"], activeTabId: "c", maximizedTabId: null },
      ],
      tabs: [
        tab("a", "w1", { command: "claude", launchId: "l1", agentId: "claude-code", agentSessionId: "s-1" }),
        tab("b", "w1", { title: "vim", botName: "Bolt" }),
        tab("c", "w2"),
      ],
    };
    const parsed = parseSession(serializeSession(data), ["w1", "w2"]);
    assert.deepEqual(parsed, { version: 1, ...data });
  });

  it("tollera garbage e versioni sconosciute", () => {
    for (const raw of [
      null,
      undefined,
      "",
      "   ",
      "non json",
      "[]",
      "null",
      "42",
      '"x"',
      "{}",
      doc({ version: 2, tabs: [tab("a", "w")] }),
      JSON.stringify({ tabs: [tab("a", "w")] }),
    ]) {
      assert.equal(parseSession(raw, ["w"]), null, String(raw));
    }
    assert.equal(parseSession(doc({ tabs: "x", groups: 5 }), ["w"]), null);
    assert.equal(parseSession(doc({ tabs: [] }), ["w"]), null);
  });

  it("scarta tab con workspace inesistente, id invalidi o duplicati", () => {
    const raw = doc({
      activeWorkspaceId: "gone",
      groups: [
        { workspaceId: "gone", order: ["x"], activeTabId: "x" },
        { workspaceId: "w", order: ["a", "bad id", "a", "x"], activeTabId: "a" },
      ],
      tabs: [
        tab("x", "gone"),
        tab("a", "w"),
        tab("bad id", "w"),
        tab("../e", "w"),
        tab("a", "w", { command: "dup" }),
        7,
        null,
        "s",
      ],
    });
    const p = parseSession(raw, ["w"]);
    assert.deepEqual(
      p.tabs.map((t) => t.tabId),
      ["a"],
    );
    assert.equal(p.tabs[0].command, "");
    assert.deepEqual(p.groups, [{ workspaceId: "w", order: ["a"], activeTabId: "a", maximizedTabId: null }]);
    assert.equal(p.activeWorkspaceId, "w");
  });

  it("ripara gruppi incoerenti: attivo/massimizzato fuori ordine, tab orfani", () => {
    const raw = doc({
      activeWorkspaceId: "w2",
      activeView: "global",
      global: { order: ["c", "a"], activeTabId: "a", maximizedTabId: "c" },
      groups: [{ workspaceId: "w1", order: ["a"], activeTabId: "zzz", maximizedTabId: "qqq" }],
      tabs: [tab("a", "w1"), tab("b", "w1"), tab("c", "w2")],
    });
    const p = parseSession(raw, ["w1", "w2"]);
    assert.deepEqual(p.groups[0], { workspaceId: "w1", order: ["a", "b"], activeTabId: "a", maximizedTabId: null });
    assert.deepEqual(p.groups[1], { workspaceId: "w2", order: ["c"], activeTabId: "c", maximizedTabId: null });
    assert.equal(p.activeWorkspaceId, "w2");
  });

  it("un tab listato nel gruppo di un altro workspace segue il proprio workspace", () => {
    const raw = doc({
      groups: [{ workspaceId: "w1", order: ["a", "b"], activeTabId: "a" }],
      tabs: [tab("a", "w1"), tab("b", "w2")],
    });
    const p = parseSession(raw, ["w1", "w2"]);
    assert.deepEqual(
      p.groups.map((g) => [g.workspaceId, g.order]),
      [
        ["w1", ["a"]],
        ["w2", ["b"]],
      ],
    );
  });

  it("ignora un agentSessionId non sicuro e tronca i campi lunghi", () => {
    const p = parseSession(
      doc({
        tabs: [tab("a", "w", { agentSessionId: "x; rm -rf /", command: "c".repeat(5000), title: "t".repeat(500) })],
      }),
      ["w"],
    );
    assert.equal(p.tabs[0].agentSessionId, undefined);
    assert.equal(p.tabs[0].command.length, 1000);
    assert.equal(p.tabs[0].title.length, 80);
  });
});

describe("sessione: id sicuri", () => {
  it("tab id", () => {
    assert.ok(isSafeTabId("3f2a-4b_c"));
    for (const bad of ["", "a b", "a/b", "a.b", "x".repeat(65), 5, null]) assert.ok(!isSafeTabId(bad), String(bad));
  });
  it("agent session id", () => {
    assert.ok(isSafeAgentSessionId("0b9e8c1a-1234-4abc-9def-0123456789ab"));
    for (const bad of ["", "a b", "a;b", "$(x)", "a`b", "x".repeat(101), undefined])
      assert.ok(!isSafeAgentSessionId(bad), String(bad));
  });
});

// Profili come li invia il backend (dati reali dell'integrazione predefinita).
const CLAUDE = {
  agent_id: "claude-code",
  is_default: true,
  session_resume: {
    command_names: ["claude", "claude.exe"],
    strip_flags: [
      { names: ["--resume", "-r"], takes_value: true },
      { names: ["--continue", "-c"], takes_value: false },
    ],
    resume_template: "{command} --resume {id}",
  },
};
// Integrazione inventata: prova che nome, flag e modello vengono dal profilo.
const FOO = {
  agent_id: "foo",
  is_default: false,
  session_resume: {
    command_names: ["foo", "Foo.cmd"],
    strip_flags: [
      { names: ["--old", "-o"], takes_value: true },
      { names: ["--again"], takes_value: false },
      { names: ["--a.b"], takes_value: false },
    ],
    resume_template: "{command} --restart {id}",
  },
};
const NO_RESUME = { agent_id: "bar", is_default: false, session_resume: null };
const PROFILES = [CLAUDE, FOO, NO_RESUME];

describe("restoreCommand", () => {
  const SID = "0b9e8c1a-1234-4abc-9def-0123456789ab";
  const restore = (tab, profiles = PROFILES) => restoreCommand(tab, profiles);

  it("senza sessione agente: comando originale", () => {
    assert.equal(restore({ command: "" }), "");
    assert.equal(restore({ command: "  npm run dev " }), "npm run dev");
    assert.equal(restore({ command: "claude" }), "claude");
  });

  it("comando dell'agente: aggiunge --resume", () => {
    assert.equal(restore({ command: "claude", agentSessionId: SID }), `claude --resume ${SID}`);
    assert.equal(
      restore({ command: "claude.exe --model opus", agentSessionId: SID }),
      `claude.exe --model opus --resume ${SID}`,
    );
    assert.equal(restore({ command: "CLAUDE", agentSessionId: SID }), `CLAUDE --resume ${SID}`);
  });

  it("toglie --resume/-r/--continue/-c esistenti", () => {
    const cases = [
      ["claude --resume old-id", "claude"],
      ["claude -r old-id --model opus", "claude --model opus"],
      ["claude --continue", "claude"],
      ["claude -c --model opus", "claude --model opus"],
      ["claude --resume=old", "claude"],
      ["claude --resume --model opus", "claude --model opus"],
      ["claude --model opus -c", "claude --model opus"],
      ['claude --append-system-prompt "a b" -r x', 'claude --append-system-prompt "a b"'],
    ];
    for (const [command, base] of cases) {
      assert.equal(restore({ command, agentSessionId: SID }), `${base} --resume ${SID}`, command);
    }
  });

  it("non tocca flag simili (--cwd, -cc)", () => {
    assert.equal(
      restore({ command: "claude --color auto", agentSessionId: SID }),
      `claude --color auto --resume ${SID}`,
    );
  });

  it("shell semplice o altro comando con sessione agente: nome canonico", () => {
    assert.equal(restore({ command: "", agentSessionId: SID }), `claude --resume ${SID}`);
    assert.equal(restore({ command: "npm run dev", agentSessionId: SID }), `claude --resume ${SID}`);
    assert.equal(restore({ command: "claudette", agentSessionId: SID }), `claude --resume ${SID}`);
    assert.equal(restore({ command: "x", agentId: "claude-code", agentSessionId: SID }), `claude --resume ${SID}`);
  });

  it("agentSessionId non sicuro: ignorato", () => {
    for (const bad of ["a; rm -rf /", "$(x)", "a b", ""]) {
      assert.equal(restore({ command: "claude -c", agentSessionId: bad }), "claude -c");
      assert.equal(restore({ command: "", agentSessionId: bad }), "");
    }
  });

  it("agentId senza profilo: comando originale", () => {
    assert.equal(restore({ command: "codex", agentId: "codex", agentSessionId: SID }), "codex");
    assert.equal(restore({ command: "claude", agentId: "codex", agentSessionId: SID }), "claude");
    assert.equal(restore({ command: "claude", agentId: "claude-code", agentSessionId: SID }, []), "claude");
  });

  it("integrazione senza contratto di ripresa: comando originale", () => {
    assert.equal(restore({ command: "bar --x", agentId: "bar", agentSessionId: SID }), "bar --x");
    assert.equal(restore({ command: "", agentId: "bar", agentSessionId: SID }), "");
    // Sessione vecchia con predefinita senza ripresa.
    const profiles = [{ ...NO_RESUME, is_default: true }, FOO];
    assert.equal(restore({ command: "npm run dev", agentSessionId: SID }, profiles), "npm run dev");
  });

  it("tab senza agentId (sessioni vecchie): riconosciuto dal comando, poi predefinito", () => {
    assert.equal(restore({ command: "foo --old x", agentSessionId: SID }), `foo --restart ${SID}`);
    assert.equal(restore({ command: "claude -c", agentSessionId: SID }), `claude --resume ${SID}`);
    // Non riconosciuto: l'integrazione predefinita.
    assert.equal(restore({ command: "npm run dev", agentSessionId: SID }), `claude --resume ${SID}`);
    assert.equal(restore({ command: "", agentSessionId: SID }, [FOO, CLAUDE]), `claude --resume ${SID}`);
    // Nessun predefinito: comando originale.
    assert.equal(restore({ command: "npm run dev", agentSessionId: SID }, [FOO]), "npm run dev");
    assert.equal(restore({ command: "", agentSessionId: SID }, []), "");
  });

  it("comando sconosciuto con agentId: nome canonico dell'integrazione del tab", () => {
    assert.equal(restore({ command: "npm run dev", agentId: "foo", agentSessionId: SID }), `foo --restart ${SID}`);
  });

  it("nulla e' cablato: nome, flag e modello vengono dal profilo", () => {
    const tab = (command) => ({ command, agentId: "foo", agentSessionId: SID });
    assert.equal(restore(tab("foo")), `foo --restart ${SID}`);
    assert.equal(restore(tab("FOO.CMD --model x")), `FOO.CMD --model x --restart ${SID}`);
    assert.equal(restore(tab("foo --old abc --model x")), `foo --model x --restart ${SID}`);
    assert.equal(restore(tab("foo -o=abc --again")), `foo --restart ${SID}`);
    assert.equal(restore(tab("foo --a.b --model x")), `foo --model x --restart ${SID}`);
    // I flag e il nome di un'altra integrazione non contano.
    assert.equal(restore(tab("foo --resume abc -c")), `foo --resume abc -c --restart ${SID}`);
    assert.equal(restore(tab("claude --old x")), `foo --restart ${SID}`);
    // I metacaratteri nei nomi dei flag non fanno da regex.
    assert.equal(restore(tab("foo --axb")), `foo --axb --restart ${SID}`);
  });

  it("`$` e segnaposto nel comando o nel modello non rompono la sostituzione", () => {
    assert.equal(
      restore({ command: 'claude --append-system-prompt "$& $1 {id}"', agentSessionId: SID }),
      `claude --append-system-prompt "$& $1 {id}" --resume ${SID}`,
    );
    const template = "$& {command} -- {id} $1";
    const profiles = [{ ...CLAUDE, session_resume: { ...CLAUDE.session_resume, resume_template: template } }];
    assert.equal(restore({ command: "claude", agentSessionId: SID }, profiles), `$& claude -- ${SID} $1`);
  });
});

describe("restoreCommand: comandi lunghi", () => {
  const SID = "0b9e8c1a-1234-4abc-9def-0123456789ab";
  it("--resume <id> non viene mai troncato", () => {
    const long = "claude --append-system-prompt " + "x".repeat(970);
    assert.equal(long.length, 1000);
    const r = restoreCommand({ command: long, agentSessionId: SID }, PROFILES);
    assert.ok(r.length > 1000);
    assert.ok(r.endsWith(` --resume ${SID}`));
  });
  it("la base e' limitata a 1000 caratteri prima del resume", () => {
    const r = restoreCommand({ command: "claude " + "y".repeat(1500), agentSessionId: SID }, PROFILES);
    assert.ok(r.endsWith(` --resume ${SID}`));
    assert.equal(r.length, 1000 + ` --resume ${SID}`.length);
  });
});

describe("debounceDelay", () => {
  it("rispetta l'attesa massima dalla prima modifica", () => {
    assert.equal(debounceDelay(1000, 1000, 500, 2000), 500);
    assert.equal(debounceDelay(2800, 1000, 500, 2000), 200);
    assert.equal(debounceDelay(3500, 1000, 500, 2000), 0);
  });
});

describe("sessione: vista globale", () => {
  const tabs = [tab("a", "w1"), tab("b", "w1"), tab("c", "w2")];

  it("sessione v1 senza campi globali: vista vuota, workspace", () => {
    const p = parseSession(doc({ tabs }), ["w1", "w2"]);
    assert.deepEqual(p.global, { order: [], activeTabId: null, maximizedTabId: null });
    assert.equal(p.activeView, "workspace");
    assert.equal(p.version, 1);
  });

  it("scarta tab ignoti, duplicati e non stringa; ripara attivo/massimizzato", () => {
    const p = parseSession(
      doc({
        tabs,
        activeView: "global",
        global: { order: ["c", "zzz", "c", 5, "a"], activeTabId: "b", maximizedTabId: "qqq" },
      }),
      ["w1", "w2"],
    );
    assert.deepEqual(p.global, { order: ["c", "a"], activeTabId: "c", maximizedTabId: null });
    assert.equal(p.activeView, "global");
  });

  it("tab di un workspace rimosso esce anche dalla vista globale", () => {
    const p = parseSession(doc({ tabs, activeView: "global", global: { order: ["c", "a"], activeTabId: "c" } }), [
      "w1",
    ]);
    assert.deepEqual(p.global.order, ["a"]);
    assert.equal(p.global.activeTabId, "a");
  });

  it("activeView global senza tab fissati: workspace; valori strani ignorati", () => {
    assert.equal(
      parseSession(doc({ tabs, activeView: "global", global: { order: [] } }), ["w1", "w2"]).activeView,
      "workspace",
    );
    assert.equal(parseSession(doc({ tabs, activeView: "boh", global: "x" }), ["w1", "w2"]).activeView, "workspace");
    assert.equal(parseSession(doc({ tabs, global: [1] }), ["w1", "w2"]).global.order.length, 0);
  });

  it("legge il nome del bot, lo pulisce e ignora i valori non validi", () => {
    const p = parseSession(
      doc({
        tabs: [
          tab("a", "w", { botName: "  Bolt " }),
          tab("b", "w", { botName: "Pi\nxel" }),
          tab("c", "w", { botName: "n".repeat(100) }),
          tab("d", "w", { botName: 42 }),
          tab("e", "w", { botName: "   " }),
          tab("f", "w"),
        ],
      }),
      ["w"],
    );
    assert.deepEqual(
      p.tabs.map((t) => t.botName),
      ["Bolt", "Pixel", "n".repeat(32), undefined, undefined, undefined],
    );
  });
});

describe("unavailableAgentsNotice", () => {
  it("nomina gli agenti delle integrazioni non piu' abilitate", () => {
    assert.equal(
      unavailableAgentsNotice(["alpha"]),
      "Integrazione disabilitata (alpha): i suoi terminali sono stati riaperti come shell.",
    );
    assert.match(unavailableAgentsNotice(["alpha", "beta"]), /(alpha, beta)/);
  });
});
