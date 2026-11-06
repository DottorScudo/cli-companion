import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { agentBotsKey, botTabSpec, botWarningsText } from "../src/lib/botLaunch.ts";
import { buildLaunchMenu } from "../src/lib/launchMenu.ts";
import { assignRestoredNames } from "../src/lib/bots.ts";
import { botLaunchesFor } from "../src/stores/config.ts";
import { createTabsStore } from "../src/stores/tabs.ts";

const botLaunch = (id, extra = {}) => ({
  id: `bot:${id}`,
  bot_id: id,
  name: id,
  agent_id: "alpha",
  agent_name: "Alpha",
  blocked_reason: null,
  ...extra,
});
const env = (envKey, readiness) => ({ readiness, status: { env_key: envKey, env_label: envKey } });
const ALPHA_INFO = (readiness) => [{ id: "alpha", name: "Alpha", envs: [env("windows", readiness)] }];
const WS = { kind: "windows" };

describe("buildLaunchMenu: Bot agente", () => {
  it("senza avvii dei Bot l'elenco e' vuoto", () => {
    assert.deepEqual(buildLaunchMenu([], [], WS).bots, []);
  });

  it("l'ordine del backend resta e l'etichetta dice con quale agente parte", () => {
    const { bots } = buildLaunchMenu([], ALPHA_INFO("ready"), WS, [
      botLaunch("zeta"),
      botLaunch("intake", { agent_id: null, agent_name: null }),
    ]);
    assert.deepEqual(
      bots.map((b) => b.label),
      ["zeta · Alpha", "intake"],
    );
    assert.ok(bots.every((b) => b.blockedReason === null));
  });

  it("il motivo del backend vince su quello di prontezza", () => {
    const { bots } = buildLaunchMenu([], ALPHA_INFO("not-found"), WS, [
      botLaunch("a", { blocked_reason: "Manca la skill." }),
    ]);
    assert.equal(bots[0].blockedReason, "Manca la skill.");
  });

  it("agente non pronto nell'ambiente: motivo di prontezza", () => {
    const { bots } = buildLaunchMenu([], ALPHA_INFO("not-found"), WS, [botLaunch("a")]);
    assert.match(bots[0].blockedReason, /Alpha non è stato trovato/);
  });

  it("agente pronto: avviabile", () => {
    assert.equal(buildLaunchMenu([], ALPHA_INFO("ready"), WS, [botLaunch("a")]).bots[0].blockedReason, null);
  });
});

describe("botLaunchesFor", () => {
  const A = [botLaunch("a")];
  const B = [botLaunch("b")];
  const state = { botLaunches: A, agentBotLaunches: { beta: B } };
  it("agente proprio: i suoi avvii; altrimenti, o se non ancora letti, quelli predefiniti", () => {
    assert.equal(botLaunchesFor(state, { agent: "beta" }), B);
    assert.equal(botLaunchesFor(state, {}), A);
    assert.equal(botLaunchesFor(state, { agent: null }), A);
    assert.equal(botLaunchesFor(state, { agent: "gamma" }), A);
  });
});

describe("nome del tab di un Bot", () => {
  const spec = (key) => ({ key, workspaceId: "w1", command: "", startCommand: "" });

  it("due tab dello stesso Bot: nome, poi nome con suffisso", () => {
    const store = createTabsStore();
    assert.equal(store.open(spec("a"), ["Intake"]).bot, "Intake");
    assert.equal(store.open(spec("b"), ["Intake"]).bot, "Intake 2");
    assert.equal(store.open(spec("c"), ["Intake"]).bot, "Intake 3");
  });

  it("un nome riservato a una segretaria prende il suffisso", () => {
    const store = createTabsStore();
    assert.equal(store.open(spec("a"), ["Intake"], ["intake"]).bot, "Intake 2");
  });

  it("i nomi degli altri tab non confliggono", () => {
    const store = createTabsStore();
    store.open(spec("a"), ["Review"]);
    assert.equal(store.open(spec("b"), ["Intake"]).bot, "Intake");
  });

  it("al ripristino il nome con suffisso resta", () => {
    const names = assignRestoredNames(
      [
        { tabId: "a", botName: "Intake" },
        { tabId: "b", botName: "Intake 2" },
      ],
      [],
    );
    assert.equal(names.get("a"), "Intake");
    assert.equal(names.get("b"), "Intake 2");
  });
});

describe("pezzi dell'avvio di un Bot", () => {
  const launch = (extra = {}) => ({
    name: "Intake",
    agent_id: "alpha",
    command: "alpha --skill intake",
    restore_command: "alpha --resume",
    warnings: [],
    ...extra,
  });

  it("botTabSpec salva il comando di ripristino ed esegue quello di avvio", () => {
    assert.deepEqual(botTabSpec("w1", "k1", { id: "bot:intake" }, launch()), {
      key: "k1",
      workspaceId: "w1",
      launchId: "bot:intake",
      agentId: "alpha",
      command: "alpha --resume",
      startCommand: "alpha --skill intake",
    });
  });

  it("botWarningsText: nessun avviso = null, altrimenti uno solo con il nome", () => {
    assert.equal(botWarningsText(launch()), null);
    assert.equal(botWarningsText(launch({ warnings: ["no effort", "no model"] })), "Bot «Intake»: no effort; no model");
  });

  it("agentBotsKey cambia con i campi che incidono sugli avvii e non con gli altri", () => {
    const bot = {
      id: "a",
      name: "A",
      task: "t",
      agent: null,
      model: { kind: "default" },
      effort: null,
      skill: null,
      enabled: true,
      image: null,
      preset: null,
    };
    const key = agentBotsKey([bot]);
    assert.equal(agentBotsKey([{ ...bot, task: "altro", image: { png: "x", color: "#000000" } }]), key);
    for (const change of [
      { name: "B" },
      { enabled: false },
      { agent: "x" },
      { effort: "high" },
      { skill: { kind: "user", id: "s" } },
    ]) {
      assert.notEqual(agentBotsKey([{ ...bot, ...change }]), key);
    }
  });
});
