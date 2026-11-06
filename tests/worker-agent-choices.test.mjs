import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  INHERIT_AGENT_VALUE,
  MODEL_DEFAULT_PLACEHOLDER,
  canRunWorkers,
  inheritAgentLabel,
  modelPlaceholder,
  modelSuggestions,
  otherConfiguredAgents,
  workerAgentOptions,
  workerAgentWarning,
} from "../src/lib/workerAgentChoices.ts";

const integration = (id, over = {}) => ({
  id,
  name: `Agent ${id.toUpperCase()}`,
  enabled: true,
  is_default: false,
  capabilities: ["automation", "models"],
  models: [],
  effort_levels: [],
  default_model: null,
  envs: [],
  ...over,
});

const integrations = [
  integration("a", { is_default: true }),
  integration("b"),
  integration("off", { enabled: false }),
  integration("manual", { capabilities: ["hook-events"] }),
];

describe("canRunWorkers", () => {
  it("serve il contratto di automazione", () => {
    assert.equal(canRunWorkers(integrations[0]), true);
    assert.equal(canRunWorkers(integrations[3]), false);
  });
});

describe("inheritAgentLabel", () => {
  it("nomina il predefinito globale", () => {
    assert.equal(inheritAgentLabel(integrations), "Eredita (Agent A)");
  });

  it("senza predefinito noto resta generico", () => {
    assert.equal(inheritAgentLabel([]), "Eredita");
    assert.equal(inheritAgentLabel([integration("a", { is_default: true, enabled: false })]), "Eredita");
  });
});

describe("workerAgentOptions", () => {
  it("Eredita e le sole integrazioni abilitate con l'automazione", () => {
    assert.deepEqual(workerAgentOptions(integrations, undefined), [
      { value: INHERIT_AGENT_VALUE, label: "Eredita (Agent A)" },
      { value: "a", label: "Agent A" },
      { value: "b", label: "Agent B" },
    ]);
  });

  it("l'agente scelto e utilizzabile non compare due volte", () => {
    assert.equal(workerAgentOptions(integrations, "b").length, 3);
  });

  it("l'agente salvato ma disabilitato resta in elenco, segnato", () => {
    const options = workerAgentOptions(integrations, "off");
    assert.deepEqual(options.at(-1), { value: "off", label: "Agent OFF (non utilizzabile)" });
  });

  it("un agente sconosciuto mostra il suo id", () => {
    assert.deepEqual(workerAgentOptions(integrations, "gone").at(-1), {
      value: "gone",
      label: "gone (non utilizzabile)",
    });
  });
});

describe("workerAgentWarning", () => {
  it("nessun avviso se il workspace eredita o l'agente è utilizzabile", () => {
    assert.equal(workerAgentWarning(undefined, integrations), null);
    assert.equal(workerAgentWarning("b", integrations), null);
  });

  it("agente disabilitato", () => {
    assert.match(workerAgentWarning("off", integrations), /Agent OFF è disabilitato/);
  });

  it("agente senza automazione", () => {
    assert.match(workerAgentWarning("manual", integrations), /Agent MANUAL non supporta i worker/);
  });

  it("agente non più registrato", () => {
    assert.match(workerAgentWarning("gone", integrations), /«gone» non è più disponibile/);
  });
});

describe("modelSuggestions", () => {
  it("etichetta e fascia di ogni modello", () => {
    const info = {
      models: [
        { id: "m-1", label: "Veloce", tier: "standard" },
        { id: "m-2", label: "Potente", tier: "strong" },
      ],
    };
    assert.deepEqual(modelSuggestions(info), [
      { id: "m-1", label: "Veloce · standard" },
      { id: "m-2", label: "Potente · avanzato" },
    ]);
  });

  it("senza integrazione o senza modelli nessun suggerimento", () => {
    assert.deepEqual(modelSuggestions(undefined), []);
    assert.deepEqual(modelSuggestions({ models: [] }), []);
  });
});

describe("modelPlaceholder", () => {
  const models = [{ id: "m-1", label: "Veloce", tier: "standard" }];

  it("senza modello predefinito dichiarato è generico", () => {
    assert.equal(modelPlaceholder(undefined), MODEL_DEFAULT_PLACEHOLDER);
    assert.equal(modelPlaceholder({ models, default_model: null }), MODEL_DEFAULT_PLACEHOLDER);
  });

  it("il predefinito si nomina con la sua etichetta, o con l'id se non è fra i suggeriti", () => {
    assert.equal(modelPlaceholder({ models, default_model: "m-1" }), `${MODEL_DEFAULT_PLACEHOLDER} (Veloce)`);
    assert.equal(modelPlaceholder({ models, default_model: "x" }), `${MODEL_DEFAULT_PLACEHOLDER} (x)`);
  });
});

describe("otherConfiguredAgents", () => {
  const profiles = [
    { agent_id: "a", agent_name: "Agent A" },
    { agent_id: "b", agent_name: "Agent B" },
    { agent_id: "c", agent_name: "Agent C" },
  ];
  const workerAgents = {
    a: { model: "m", allow: [], deny: [] },
    b: { model: null, allow: [], deny: ["x"] },
    c: { model: null, allow: [], deny: [] },
    gone: { model: "m", allow: [], deny: [] },
  };

  it("gli altri agenti con modello o regole, nell'ordine dei profili", () => {
    assert.deepEqual(otherConfiguredAgents(workerAgents, "a", profiles), [{ id: "b", name: "Agent B" }]);
    assert.deepEqual(otherConfiguredAgents(workerAgents, "c", profiles), [
      { id: "a", name: "Agent A" },
      { id: "b", name: "Agent B" },
    ]);
  });

  it("un agente senza voce o con voce vuota non compare; neanche quello fuori dai profili", () => {
    assert.deepEqual(otherConfiguredAgents({}, "a", profiles), []);
    assert.deepEqual(
      otherConfiguredAgents(workerAgents, undefined, profiles).map((a) => a.id),
      ["a", "b"],
    );
  });
});
