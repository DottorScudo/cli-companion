import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { botLaunchLabel, buildLaunchMenu, launchBlockReason } from "../src/lib/launchMenu.ts";
import { defaultLaunchFor, sharedLaunchesFor } from "../src/stores/config.ts";

const agentLaunch = (agent, name) => ({
  id: `builtin:${agent}`,
  name,
  command: agent,
  restore_command: agent,
  agent_id: agent,
  agent_name: name,
});
const botLaunch = (id, name, agent, agentName) => ({
  id: `bot:${id}`,
  bot_id: id,
  name,
  agent_id: agent,
  agent_name: agentName,
  blocked_reason: null,
});

const ALPHA = agentLaunch("alpha", "Alpha");
const BETA = agentLaunch("beta", "Beta");
const BOTH = [ALPHA, BETA];
const INTAKE = botLaunch("intake", "Intake", "alpha", "Alpha");
const INTAKE_BETA = botLaunch("intake-beta", "Intake (Beta)", "beta", "Beta");

const WS = { kind: "windows" };
const WSL_WS = { kind: "wsl", distro: "Ubuntu" };

const env = (envKey, readiness, label = envKey) => ({ readiness, status: { env_key: envKey, env_label: label } });
const info = (id, name, envs) => ({ id, name, envs });

describe("buildLaunchMenu", () => {
  it("un avvio per ogni integrazione abilitata, con il suo nome, e nessuna voce dei preset", () => {
    const menu = buildLaunchMenu(BOTH, [], WS);
    assert.deepEqual(
      menu.entries.map((e) => e.label),
      ["Alpha", "Beta"],
    );
    assert.ok(menu.entries.every((e) => e.blockedReason === null));
    assert.deepEqual(Object.keys(menu).sort(), ["bots", "entries"], "niente sottomenu «Avvia con…»");
  });

  it("i Bot coprono i casi dei preset: anche con l'agente fissato", () => {
    const { bots } = buildLaunchMenu(BOTH, [], WS, [INTAKE, INTAKE_BETA]);
    assert.deepEqual(
      bots.map((b) => [b.label, b.launch.id]),
      [
        ["Intake · Alpha", "bot:intake"],
        ["Intake (Beta) · Beta", "bot:intake-beta"],
      ],
    );
  });

  it("nessun avvio: menu vuoto", () => {
    assert.deepEqual(buildLaunchMenu([], [], WS), { entries: [], bots: [] });
  });

  it("l'integrazione non pronta nell'ambiente del workspace resta in elenco, con il motivo", () => {
    const integrations = [
      info("alpha", "Alpha", [env("windows", "ready"), env("Ubuntu", "not-installed", "WSL Ubuntu")]),
      info("beta", "Beta", [env("windows", "not-found", "Windows"), env("Ubuntu", "ready", "WSL Ubuntu")]),
    ];
    const windows = buildLaunchMenu(BOTH, integrations, WS, [INTAKE_BETA]);
    assert.deepEqual(
      windows.entries.map((e) => e.blockedReason === null),
      [true, false],
    );
    assert.equal(
      windows.entries[1].blockedReason,
      "Beta non è stato trovato in Windows: installalo dalle Integrazioni.",
    );
    assert.ok(windows.bots[0].blockedReason?.startsWith("Beta non è stato trovato"));
    const wsl = buildLaunchMenu(BOTH, integrations, WSL_WS);
    assert.equal(
      wsl.entries[0].blockedReason,
      "Il pacchetto di Alpha non è installato in WSL Ubuntu: installalo dalle Integrazioni.",
    );
    assert.equal(wsl.entries[1].blockedReason, null);
  });
});

describe("botLaunchLabel", () => {
  it("dice l'agente solo se si è potuto risolvere", () => {
    assert.equal(botLaunchLabel(INTAKE), "Intake · Alpha");
    assert.equal(botLaunchLabel({ name: "Setup", agent_name: null }), "Setup");
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

describe("avvio predefinito con i Bot", () => {
  const state = {
    builtinLaunches: [ALPHA],
    botLaunches: [INTAKE],
    agentBotLaunches: { beta: [INTAKE_BETA] },
    globalLaunch: "bot:intake",
  };

  it("gli avvii condivisi sono gli integrati e i Bot dell'agente del workspace", () => {
    assert.deepEqual(sharedLaunchesFor(state, {}), [ALPHA, INTAKE]);
    assert.deepEqual(sharedLaunchesFor(state, { agent: "beta" }), [ALPHA, INTAKE_BETA]);
    assert.deepEqual(sharedLaunchesFor(state, { agent: "gamma" }), [ALPHA, INTAKE], "non ancora letti: senza agente");
  });

  it("un Bot può essere il predefinito globale o del workspace", () => {
    assert.equal(defaultLaunchFor(state, { default_launch: null }), INTAKE);
    assert.equal(defaultLaunchFor(state, { agent: "beta", default_launch: "bot:intake-beta" }), INTAKE_BETA);
    assert.equal(defaultLaunchFor(state, { default_launch: "builtin:alpha" }), ALPHA);
  });

  it("un Bot sparito scelto dal workspace non ripiega sul globale", () => {
    assert.equal(defaultLaunchFor(state, { default_launch: "bot:gone" }), undefined);
  });
});
