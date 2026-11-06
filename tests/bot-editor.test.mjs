import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  EFFORT_IGNORED_NOTE,
  FOLLOW_WORKSPACE,
  agentChoices,
  botAgents,
  botSummary,
  draftFromBot,
  draftFromForm,
  effortChoices,
  emptyBotForm,
  fieldOfBackendError,
  formAfterAgentChange,
  formFromBot,
  isFormDirty,
  isGeneralBot,
  skillFromValue,
  skillValue,
  validateBotForm,
} from "../src/lib/botEditor.ts";

const integration = (id, over = {}) => ({
  id,
  name: id.toUpperCase(),
  enabled: true,
  is_default: false,
  capabilities: ["automation", "interactive-launch", "models", "effort"],
  models: [{ id: `${id}-big`, label: "Big" }],
  default_model: null,
  effort_levels: ["low", "high"],
  envs: [],
  ...over,
});
const WITH_EFFORT = integration("alpha");
const NO_EFFORT = integration("beta", {
  effort_levels: [],
  capabilities: ["automation", "interactive-launch"],
  models: [],
});
const OFF = integration("gamma", { enabled: false });
const NO_BOT = integration("delta", { capabilities: ["automation"] });
const ALL = [WITH_EFFORT, NO_EFFORT, OFF, NO_BOT];

const bot = (over = {}) => ({
  id: "custom-x",
  name: "Reviewer",
  task: "Reviews diffs.",
  agent: null,
  model: { kind: "default" },
  effort: null,
  skill: null,
  enabled: true,
  image: null,
  preset: null,
  ...over,
});
const NONE_TAKEN = { bots: [], secretaries: [] };

describe("botEditor: Bot e modulo", () => {
  it("il modulo di un Bot e la sua bozza si corrispondono", () => {
    const b = bot({
      agent: "alpha",
      model: { kind: "tier", value: "strong" },
      effort: "high",
      skill: { kind: "user", id: "mine" },
    });
    assert.deepEqual(draftFromBot(b), {
      name: "Reviewer",
      task: "Reviews diffs.",
      agent: "alpha",
      model: { kind: "tier", value: "strong" },
      effort: "high",
      skill: { kind: "user", id: "mine" },
      enabled: true,
      image: null,
    });
  });

  it("modello preciso e predefinito passano dal modulo", () => {
    assert.equal(formFromBot(bot({ model: { kind: "id", value: "m-1" } })).modelKind, "id");
    const form = { ...emptyBotForm(), name: " A ", task: " t ", agent: "alpha", modelKind: "id", modelId: " m-1 " };
    const draft = draftFromForm(form);
    assert.deepEqual(draft.model, { kind: "id", value: "m-1" });
    assert.equal(draft.name, "A");
    assert.deepEqual(draftFromForm(emptyBotForm()).model, { kind: "default" });
    assert.equal(draftFromForm(emptyBotForm()).agent, null);
  });

  it("segue il workspace: agente vuoto nel modulo, null nella bozza", () => {
    assert.equal(formFromBot(bot()).agent, FOLLOW_WORKSPACE);
  });

  it("riconosce il Bot General", () => {
    assert.equal(isGeneralBot({ id: "general" }), true);
    assert.equal(isGeneralBot(bot()), false);
  });

  it("isFormDirty confronta i campi e l'immagine per contenuto", () => {
    const saved = formFromBot(bot({ image: { png: "AAAA", color: "#000000" } }));
    assert.equal(isFormDirty({ ...saved, image: { png: "AAAA", color: "#000000" } }, saved), false);
    assert.equal(isFormDirty({ ...saved, image: null }, saved), true);
    assert.equal(isFormDirty({ ...saved, task: "altro" }, saved), true);
  });
});

describe("botEditor: skill", () => {
  it("i valori delle opzioni fanno il giro", () => {
    for (const skill of [null, { kind: "plugin", id: "a" }, { kind: "user", id: "b:c" }]) {
      assert.deepEqual(skillFromValue(skillValue(skill)), skill);
    }
  });

  it("un valore non riconosciuto vale nessuna skill", () => {
    assert.equal(skillFromValue("nope"), null);
    assert.equal(skillFromValue("other:x"), null);
    assert.equal(skillFromValue("user:"), null);
  });
});

describe("botEditor: agenti", () => {
  it("fissabili solo gli agenti abilitati con i contratti dei Bot", () => {
    assert.deepEqual(
      botAgents(ALL).map((i) => i.id),
      ["alpha", "beta"],
    );
  });

  it("il selettore parte da «Segue il workspace»", () => {
    const choices = agentChoices(ALL, FOLLOW_WORKSPACE);
    assert.deepEqual(
      choices.map((c) => c.id),
      [FOLLOW_WORKSPACE, "alpha", "beta"],
    );
  });

  it("un agente poi disabilitato resta visibile, marcato non disponibile", () => {
    const choices = agentChoices(ALL, "gamma");
    assert.equal(choices.at(-1).id, "gamma");
    assert.match(choices.at(-1).label, /GAMMA.*non disponibile/);
  });
});

describe("botEditor: effort", () => {
  it("nascosto per un agente che non lo supporta", () => {
    assert.deepEqual(effortChoices(ALL, "beta"), { show: false });
    assert.deepEqual(effortChoices(ALL, "sconosciuto"), { show: false });
  });

  it("agente fissato: i suoi livelli, in ordine, senza nota", () => {
    assert.deepEqual(effortChoices(ALL, "alpha"), { show: true, levels: ["low", "high"], note: null });
  });

  it("segue il workspace: i livelli dei fissabili, con la nota", () => {
    assert.deepEqual(effortChoices(ALL, FOLLOW_WORKSPACE), {
      show: true,
      levels: ["low", "high"],
      note: EFFORT_IGNORED_NOTE,
    });
  });

  it("segue il workspace senza nessun agente con effort: nascosto", () => {
    assert.deepEqual(effortChoices([NO_EFFORT], FOLLOW_WORKSPACE), { show: false });
  });

  it("cambiando agente cade l'effort non supportato", () => {
    const form = { ...emptyBotForm(), agent: "alpha", effort: "high" };
    assert.equal(formAfterAgentChange(ALL, form, "beta").effort, "");
    assert.equal(formAfterAgentChange(ALL, form, FOLLOW_WORKSPACE).effort, "high");
    assert.equal(formAfterAgentChange(ALL, { ...form, effort: "max" }, "alpha").effort, "");
  });
});

describe("botEditor: modello", () => {
  const form = { ...emptyBotForm(), agent: "alpha", modelKind: "id", modelId: "alpha-big" };

  it("il modello preciso resta se il nuovo agente lo consiglia", () => {
    assert.equal(formAfterAgentChange(ALL, form, "alpha").modelId, "alpha-big");
  });

  it("altrimenti torna al predefinito", () => {
    for (const agent of ["beta", FOLLOW_WORKSPACE]) {
      const next = formAfterAgentChange(ALL, form, agent);
      assert.equal(next.modelKind, "default");
      assert.equal(next.modelId, "");
    }
  });

  it("le fasce non dipendono dall'agente", () => {
    const next = formAfterAgentChange(ALL, { ...form, modelKind: "strong" }, FOLLOW_WORKSPACE);
    assert.equal(next.modelKind, "strong");
  });
});

describe("botEditor: validazione", () => {
  const valid = { ...emptyBotForm(), name: "Reviewer", task: "Reviews diffs." };

  it("un modulo valido non ha errori", () => {
    assert.deepEqual(validateBotForm(valid, NONE_TAKEN), {});
  });

  it("nome vuoto, troppo lungo o con caratteri di controllo", () => {
    assert.ok(validateBotForm({ ...valid, name: "  " }, NONE_TAKEN).name);
    assert.ok(validateBotForm({ ...valid, name: "x".repeat(25) }, NONE_TAKEN).name);
    assert.ok(validateBotForm({ ...valid, name: "a\u0007b" }, NONE_TAKEN).name);
    assert.equal(validateBotForm({ ...valid, name: "😀".repeat(24) }, NONE_TAKEN).name, undefined);
  });

  it("nome già di un altro Bot o di un segretario, senza badare alle maiuscole", () => {
    assert.match(validateBotForm(valid, { bots: ["REVIEWER"], secretaries: [] }).name, /già un Bot/);
    assert.match(validateBotForm(valid, { bots: [], secretaries: ["reviewer"] }).name, /segretario/);
  });

  it("compito vuoto, troppo lungo o su più righe", () => {
    assert.ok(validateBotForm({ ...valid, task: "" }, NONE_TAKEN).task);
    assert.ok(validateBotForm({ ...valid, task: "x".repeat(301) }, NONE_TAKEN).task);
    assert.ok(validateBotForm({ ...valid, task: "a\nb" }, NONE_TAKEN).task);
  });

  it("il modello preciso vuole l'agente fissato e un nome valido", () => {
    const precise = { ...valid, modelKind: "id", modelId: "m-1" };
    assert.ok(validateBotForm(precise, NONE_TAKEN).model);
    assert.equal(validateBotForm({ ...precise, agent: "alpha" }, NONE_TAKEN).model, undefined);
    assert.ok(validateBotForm({ ...precise, agent: "alpha", modelId: "" }, NONE_TAKEN).model);
    assert.ok(validateBotForm({ ...precise, agent: "alpha", modelId: "-bad" }, NONE_TAKEN).model);
  });
});

describe("botEditor: errori del backend", () => {
  it("assegna ogni errore al suo campo", () => {
    const cases = [
      ["il nome deve avere da 1 a 24 caratteri", "name"],
      ["c'è già un Bot di nome “A”", "name"],
      ["“A” è già il nome di un segretario", "name"],
      ["il compito deve avere da 1 a 300 caratteri", "task"],
      ["Alpha non supporta l'effort “max”", "effort"],
      ["skill del plugin sconosciuta: “x”", "skill"],
      ["modello non valido per Alpha: “x”", "model"],
      ["per scegliere un modello preciso fissa prima l'agente del Bot", "model"],
      ["Alpha è disattivato: abilitalo nelle integrazioni", "agent"],
      ["agente sconosciuto: zzz", "agent"],
    ];
    for (const [message, field] of cases) assert.equal(fieldOfBackendError(message), field, message);
  });

  it("un errore non riconosciuto resta generale", () => {
    assert.equal(fieldOfBackendError("disco pieno"), null);
  });
});

describe("botEditor: riepilogo", () => {
  const skills = [{ skill: { kind: "plugin", id: "p" }, name: "Plan", description: "" }];

  it("Bot semplice: agente del workspace e modello predefinito", () => {
    assert.deepEqual(botSummary(bot(), ALL, skills), ["Segue il workspace", "Predefinito dell’agente"]);
  });

  it("Bot completo: agente, modello, effort e skill", () => {
    const b = bot({
      agent: "alpha",
      model: { kind: "id", value: "alpha-big" },
      effort: "high",
      skill: { kind: "plugin", id: "p" },
    });
    assert.deepEqual(botSummary(b, ALL, skills), ["ALPHA", "alpha-big", "Effort alta", "Plan"]);
  });

  it("agente o skill sconosciuti si mostrano con l'id", () => {
    const b = bot({ agent: "zzz", model: { kind: "tier", value: "standard" }, skill: { kind: "user", id: "mine" } });
    assert.deepEqual(botSummary(b, ALL, []), ["zzz", "Standard", "mine"]);
  });
});
