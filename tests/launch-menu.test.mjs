import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { buildLaunchMenu, distinctLaunches, launchBlockReason, launchLabel } from "../src/lib/launchMenu.ts";
import { launchesFor } from "../src/stores/config.ts";

const agentLaunch = (agent, name) => ({
  id: `builtin:${agent}`,
  name,
  command: agent,
  restore_command: agent,
  agent_id: agent,
  agent_name: name,
  preset: null,
  follows_workspace_agent: false,
});
const presetLaunch = (preset, agent, agentName, follows) => ({
  id: follows ? `builtin:preset:${preset}` : `builtin:preset:${preset}@${agent}`,
  name: preset === "task-intake" ? "Task Intake" : "Skill Wizard",
  command: `${agent} run ${preset}`,
  restore_command: agent,
  agent_id: agent,
  agent_name: agentName,
  preset,
  follows_workspace_agent: follows,
});

// Due agenti che dichiarano i preset; "alpha" e' quello del workspace.
const ALPHA = agentLaunch("alpha", "Alpha");
const BETA = agentLaunch("beta", "Beta");
const BOTH = [
  ALPHA,
  BETA,
  presetLaunch("task-intake", "alpha", "Alpha", true),
  presetLaunch("skill-wizard", "alpha", "Alpha", true),
  presetLaunch("task-intake", "alpha", "Alpha", false),
  presetLaunch("skill-wizard", "alpha", "Alpha", false),
  presetLaunch("task-intake", "beta", "Beta", false),
  presetLaunch("skill-wizard", "beta", "Beta", false),
];
const SINGLE = [
  ALPHA,
  presetLaunch("task-intake", "alpha", "Alpha", true),
  presetLaunch("task-intake", "alpha", "Alpha", false),
];

const WS = { kind: "windows" };
const WSL_WS = { kind: "wsl", distro: "Ubuntu" };

const env = (envKey, readiness, label = envKey) => ({ readiness, status: { env_key: envKey, env_label: label } });
const info = (id, name, envs) => ({ id, name, envs });

describe("launchLabel", () => {
  it("un preset dice con quale agente parte; l'avvio dell'agente e quelli personalizzati hanno il loro nome", () => {
    assert.equal(launchLabel(presetLaunch("task-intake", "alpha", "Alpha", true)), "Task Intake · Alpha");
    assert.equal(launchLabel(presetLaunch("task-intake", "beta", "Beta", false)), "Task Intake · Beta");
    assert.equal(launchLabel(ALPHA), "Alpha");
    assert.equal(launchLabel({ id: "l1", name: "Dev server", command: "npm run dev" }), "Dev server");
  });
});

describe("buildLaunchMenu", () => {
  it("un avvio per ogni integrazione abilitata, poi i preset con l'agente del workspace", () => {
    const { entries } = buildLaunchMenu(BOTH, [], WS);
    assert.deepEqual(
      entries.map((e) => e.label),
      ["Alpha", "Beta", "Task Intake · Alpha", "Skill Wizard · Alpha"],
    );
    assert.ok(entries.every((e) => e.blockedReason === null));
  });

  it('"Avvia con…" elenca solo gli altri agenti, raggruppati, con il nome del preset', () => {
    const { others } = buildLaunchMenu(BOTH, [], WS);
    assert.equal(others.length, 1);
    assert.equal(others[0].agentId, "beta");
    assert.equal(others[0].agentName, "Beta");
    assert.deepEqual(
      others[0].entries.map((e) => [e.label, e.launch.id]),
      [
        ["Task Intake", "builtin:preset:task-intake@beta"],
        ["Skill Wizard", "builtin:preset:skill-wizard@beta"],
      ],
    );
  });

  it("con una sola integrazione il sottomenu e' vuoto", () => {
    const { entries, others } = buildLaunchMenu(SINGLE, [], WS);
    assert.deepEqual(
      entries.map((e) => e.label),
      ["Alpha", "Task Intake · Alpha"],
    );
    assert.deepEqual(others, []);
  });

  it("senza un agente risolto per i preset restano gli avvii e tutti gli agenti nel sottomenu", () => {
    const unresolved = BOTH.filter((l) => !l.follows_workspace_agent);
    const { entries, others } = buildLaunchMenu(unresolved, [], WS);
    assert.deepEqual(
      entries.map((e) => e.label),
      ["Alpha", "Beta"],
    );
    assert.deepEqual(
      others.map((a) => a.agentId),
      ["alpha", "beta"],
    );
  });

  it("nessun avvio: menu vuoto", () => {
    assert.deepEqual(buildLaunchMenu([], [], WS), { entries: [], others: [], bots: [] });
  });

  it("l'integrazione non pronta nell'ambiente del workspace resta in elenco, con il motivo", () => {
    const integrations = [
      info("alpha", "Alpha", [env("windows", "ready"), env("Ubuntu", "not-installed", "WSL Ubuntu")]),
      info("beta", "Beta", [env("windows", "not-found", "Windows"), env("Ubuntu", "ready", "WSL Ubuntu")]),
    ];
    const windows = buildLaunchMenu(BOTH, integrations, WS);
    assert.deepEqual(
      windows.entries.map((e) => e.blockedReason === null),
      [true, false, true, true],
    );
    assert.equal(
      windows.entries[1].blockedReason,
      "Beta non è stato trovato in Windows: installalo dalle Integrazioni.",
    );
    assert.ok(windows.others[0].entries.every((e) => e.blockedReason?.startsWith("Beta non è stato trovato")));
    const wsl = buildLaunchMenu(BOTH, integrations, WSL_WS);
    assert.equal(
      wsl.entries[0].blockedReason,
      "Il pacchetto di Alpha non è installato in WSL Ubuntu: installalo dalle Integrazioni.",
    );
    assert.equal(wsl.entries[1].blockedReason, null);
  });
});

describe("launchBlockReason", () => {
  const integrations = [
    info("alpha", "Alpha", [env("windows", "outdated", "Windows"), env("Ubuntu", "ready"), env("Debian", "disabled")]),
  ];
  it("un motivo per ogni stato non pronto", () => {
    assert.equal(
      launchBlockReason(ALPHA, integrations, "windows"),
      "Il pacchetto di Alpha in Windows è da aggiornare: aggiornalo dalle Integrazioni.",
    );
  });
  it("pronto, ambiente o integrazione sconosciuti, o elenco non ancora caricato: nessun blocco", () => {
    assert.equal(launchBlockReason(ALPHA, integrations, "Ubuntu"), null);
    assert.equal(launchBlockReason(ALPHA, integrations, "Debian"), null);
    assert.equal(launchBlockReason(ALPHA, integrations, "Fedora"), null);
    assert.equal(launchBlockReason(BETA, integrations, "windows"), null);
    assert.equal(launchBlockReason(ALPHA, [], "windows"), null);
  });
});

describe("distinctLaunches", () => {
  it("toglie la voce con l'agente scelto uguale a quello risolto: sarebbe un doppione", () => {
    assert.deepEqual(
      distinctLaunches(BOTH).map((l) => l.id),
      [
        "builtin:alpha",
        "builtin:beta",
        "builtin:preset:task-intake",
        "builtin:preset:skill-wizard",
        "builtin:preset:task-intake@beta",
        "builtin:preset:skill-wizard@beta",
      ],
    );
    assert.deepEqual(
      distinctLaunches(SINGLE).map((l) => l.id),
      ["builtin:alpha", "builtin:preset:task-intake"],
    );
  });
  it("senza agente risolto restano tutte le voci con l'agente scelto", () => {
    const unresolved = BOTH.filter((l) => !l.follows_workspace_agent);
    assert.equal(distinctLaunches(unresolved).length, unresolved.length);
  });
});

describe("launchesFor", () => {
  const state = { builtinLaunches: [ALPHA], agentLaunches: { beta: [BETA] } };
  it("un workspace con un agente proprio usa i suoi avvii, gli altri quelli senza agente", () => {
    assert.deepEqual(launchesFor(state, { agent: "beta" }), [BETA]);
    assert.deepEqual(launchesFor(state, {}), [ALPHA]);
    assert.deepEqual(launchesFor(state, { agent: null }), [ALPHA]);
  });
  it("avvii dell'agente non ancora letti: ripiega su quelli senza agente", () => {
    assert.deepEqual(launchesFor(state, { agent: "gamma" }), [ALPHA]);
  });
});
