import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  GENERAL_BOT_ID,
  NO_BOT_LABEL,
  UNSELECTABLE_BOT_SUFFIX,
  botAgentId,
  botById,
  botName,
  botOptions,
  defaultBotChoice,
} from "../src/lib/taskBot.ts";
import { taskAgents } from "../src/lib/taskAgent.ts";
import { botChange, buildPatch, formFromTask } from "../src/lib/taskForm.ts";

const bot = (id, over = {}) => ({
  id,
  name: `Bot ${id}`,
  task: "Does things",
  agent: null,
  model: { kind: "inherit" },
  effort: null,
  skill: null,
  enabled: true,
  image: null,
  preset: null,
  ...over,
});

const GENERAL = bot(GENERAL_BOT_ID, { name: "Generale" });
const REVIEWER = bot("reviewer", { name: "Revisore", agent: "beta" });
const OFF = bot("off", { name: "Spento", enabled: false });
const BOTS = [REVIEWER, OFF, GENERAL];

describe("ricerca del Bot", () => {
  it("botById: il Bot se esiste, altrimenti niente", () => {
    assert.equal(botById(BOTS, "reviewer"), REVIEWER);
    assert.equal(botById(BOTS, "ignoto"), undefined);
    assert.equal(botById(BOTS, undefined), undefined);
    assert.equal(botById(BOTS, ""), undefined);
  });
  it("botName: il nome, l'id se il Bot è stato cancellato, null senza Bot", () => {
    assert.equal(botName(BOTS, "reviewer"), "Revisore");
    assert.equal(botName(BOTS, "cancellato"), "cancellato");
    assert.equal(botName(BOTS, undefined), null);
    assert.equal(botName(BOTS, ""), null);
  });
});

describe("scelta del Bot nei form", () => {
  it("solo i Bot abilitati, nell'ordine dei dati", () => {
    assert.deepEqual(botOptions(BOTS, "reviewer"), [
      { id: "reviewer", label: "Revisore" },
      { id: GENERAL_BOT_ID, label: "Generale" },
    ]);
  });
  it("un Bot disabilitato già assegnato resta in coda, marcato", () => {
    const options = botOptions(BOTS, "off");
    assert.deepEqual(options.at(-1), { id: "off", label: `Spento ${UNSELECTABLE_BOT_SUFFIX}` });
    assert.equal(options.length, 3);
  });
  it("un Bot cancellato resta in coda col suo id", () => {
    assert.deepEqual(botOptions(BOTS, "cancellato").at(-1), {
      id: "cancellato",
      label: `cancellato ${UNSELECTABLE_BOT_SUFFIX}`,
    });
  });
  it("«Nessun Bot» c'è solo per chi non ne ha uno", () => {
    assert.deepEqual(botOptions(BOTS, "")[0], { id: "", label: NO_BOT_LABEL });
    assert.ok(!botOptions(BOTS, "reviewer").some((o) => o.id === ""));
  });
  it("senza Bot: solo «Nessun Bot»", () => {
    assert.deepEqual(botOptions([], ""), [{ id: "", label: NO_BOT_LABEL }]);
  });
  it("preselezione: general, altrimenti il primo abilitato, altrimenti niente", () => {
    assert.equal(defaultBotChoice(BOTS), GENERAL_BOT_ID);
    assert.equal(defaultBotChoice([OFF, REVIEWER]), "reviewer");
    assert.equal(defaultBotChoice([bot(GENERAL_BOT_ID, { enabled: false }), REVIEWER]), "reviewer");
    assert.equal(defaultBotChoice([OFF]), "");
    assert.equal(defaultBotChoice([]), "");
  });
});

describe("agente del Bot", () => {
  it("il suo, altrimenti quello del workspace", () => {
    assert.equal(botAgentId(BOTS, "reviewer", "alpha"), "beta");
    assert.equal(botAgentId(BOTS, GENERAL_BOT_ID, "alpha"), "alpha");
  });
  it("Bot ignoto o niente agente noto: vuoto", () => {
    assert.equal(botAgentId(BOTS, "ignoto", "alpha"), "");
    assert.equal(botAgentId(BOTS, GENERAL_BOT_ID, undefined), "");
  });
});

describe("i Bot dei task", () => {
  it("taskAgents li porta con sé; senza, la lista è vuota", () => {
    const ws = { kind: "windows", worker_agents: {} };
    assert.deepEqual(taskAgents([], ws, BOTS).bots, BOTS);
    assert.deepEqual(taskAgents([], ws).bots, []);
  });
});

describe("Bot nella patch del form", () => {
  const stored = {
    id: "t",
    title: "T",
    task: "S",
    depends_on: [],
    repo: null,
    agent: "alpha",
    bot: GENERAL_BOT_ID,
    model: "a-small",
    kind: "work",
  };
  const legacy = { ...stored, bot: undefined };

  it("il form parte dal Bot del task, vuoto per i task vecchi", () => {
    assert.equal(formFromTask(stored).bot, GENERAL_BOT_ID);
    assert.equal(formFromTask(legacy).bot, "");
  });
  it("Bot invariato: nessun campo bot", () => {
    assert.deepEqual(buildPatch(stored, formFromTask(stored), false), {});
    assert.deepEqual(buildPatch(legacy, formFromTask(legacy), false), {});
  });
  it("cambiare Bot svuota il modello e il backend riallinea il resto: solo bot e model", () => {
    const form = { ...formFromTask(stored), ...botChange("reviewer") };
    assert.deepEqual(buildPatch(stored, form, false), { bot: "reviewer", model: null });
  });
  it("cambiare Bot e scrivere un modello: il modello è un override nella patch", () => {
    const form = { ...formFromTask(stored), ...botChange("reviewer"), model: "b-only" };
    assert.deepEqual(buildPatch(stored, form, false), { bot: "reviewer", model: "b-only" });
  });
  it("un task vecchio che prende un Bot", () => {
    const form = { ...formFromTask(legacy), ...botChange("reviewer") };
    assert.deepEqual(buildPatch(legacy, form, false), { bot: "reviewer", model: null });
  });
  it("tornare a «Nessun Bot» azzera il Bot", () => {
    const form = { ...formFromTask(stored), bot: "" };
    assert.deepEqual(buildPatch(stored, form, false), { bot: null });
  });
  it("botChange non tocca l'agente", () => {
    assert.deepEqual(botChange("reviewer"), { bot: "reviewer", model: "" });
  });
});
