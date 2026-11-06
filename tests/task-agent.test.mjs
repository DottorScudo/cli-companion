import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_MODEL_PLACEHOLDER,
  UNSELECTABLE_AGENT_SUFFIX,
  WORKSPACE_AGENT_SUFFIX,
  agentBadge,
  agentFilterOptions,
  agentName,
  agentOptions,
  defaultAgentChoice,
  modelAfterAgentChange,
  modelPlaceholder,
  modelSuggestions,
  proposalAgentLabel,
  showAgentFilter,
  taskAgents,
  unavailableDetail,
  unavailableText,
  workspaceEnvLabel,
} from "../src/lib/taskAgent.ts";
import { buildPatch, formFromTask } from "../src/lib/taskForm.ts";

const model = (id, tier = "standard") => ({ id, label: `Modello ${id}`, tier });

const integration = (id, over = {}) => ({
  id,
  name: `Agente ${id}`,
  enabled: true,
  is_default: false,
  capabilities: ["automation", "models"],
  models: [],
  effort_levels: [],
  default_model: null,
  envs: [],
  ...over,
});

const ALPHA = integration("alpha", { is_default: true, models: [model("a-small"), model("a-big", "strong")] });
const BETA = integration("beta", { models: [model("b-only")], default_model: "b-only" });
const GAMMA = integration("gamma", { capabilities: ["hook-events"] });
const OFF = integration("off", { enabled: false });

const workspace = (over = {}) => ({ kind: "windows", worker_agents: {}, ...over });
const agentsOf = (integrations, ws = workspace()) => taskAgents(integrations, ws);

describe("taskAgents", () => {
  it("scelte: abilitate e con i worker, nell'ordine delle integrazioni", () => {
    const a = agentsOf([BETA, ALPHA, GAMMA, OFF]);
    assert.deepEqual(
      a.choices.map((i) => i.id),
      ["beta", "alpha"],
    );
  });
  it("agente del workspace: il suo, altrimenti il predefinito globale", () => {
    assert.equal(agentsOf([ALPHA, BETA]).workspaceAgentId, "alpha");
    assert.equal(agentsOf([ALPHA, BETA], workspace({ agent: "beta" })).workspaceAgentId, "beta");
    assert.equal(agentsOf([integration("solo")]).workspaceAgentId, undefined);
  });
  it("più integrazioni abilitate: contano anche quelle senza worker, non le disabilitate", () => {
    assert.equal(agentsOf([ALPHA]).multiple, false);
    assert.equal(agentsOf([ALPHA, OFF]).multiple, false);
    assert.equal(agentsOf([ALPHA, GAMMA]).multiple, true);
  });
  it("pronte solo con le integrazioni caricate", () => {
    assert.equal(agentsOf([]).ready, false);
    assert.equal(agentsOf([ALPHA]).ready, true);
  });
  it("nomi di tutte le integrazioni, anche disabilitate; id se sconosciuta", () => {
    const a = agentsOf([ALPHA, OFF]);
    assert.equal(agentName(a, "off"), "Agente off");
    assert.equal(agentName(a, "ignoto"), "ignoto");
  });
  it("ambiente del workspace: Windows o la distro WSL", () => {
    assert.equal(workspaceEnvLabel({ kind: "windows" }), "Windows");
    assert.equal(workspaceEnvLabel({ kind: "wsl", distro: "Ubuntu-24.04" }), "Ubuntu-24.04");
    assert.equal(workspaceEnvLabel({ kind: "wsl" }), "Windows");
  });
});

describe("scelta dell'agente nei form", () => {
  it("preselezione: l'agente del workspace, altrimenti il primo, altrimenti niente", () => {
    assert.equal(defaultAgentChoice(agentsOf([BETA, ALPHA])), "alpha");
    assert.equal(defaultAgentChoice(agentsOf([ALPHA, BETA], workspace({ agent: "beta" }))), "beta");
    // L'agente del workspace non è selezionabile (disabilitato): si propone il primo disponibile.
    assert.equal(defaultAgentChoice(agentsOf([OFF, BETA], workspace({ agent: "off" }))), "beta");
    assert.equal(defaultAgentChoice(agentsOf([])), "");
    assert.equal(defaultAgentChoice(agentsOf([OFF])), "");
  });
  it("voci: l'agente del workspace è marcato", () => {
    const options = agentOptions(agentsOf([ALPHA, BETA]), "alpha");
    assert.deepEqual(options, [
      { id: "alpha", label: `Agente alpha ${WORKSPACE_AGENT_SUFFIX}` },
      { id: "beta", label: "Agente beta" },
    ]);
  });
  it("voci: l'agente corrente non selezionabile resta in coda, una volta sola", () => {
    const a = agentsOf([ALPHA, OFF]);
    const options = agentOptions(a, "off");
    assert.deepEqual(
      options.map((o) => o.id),
      ["alpha", "off"],
    );
    assert.equal(options[1].label, `Agente off ${UNSELECTABLE_AGENT_SUFFIX}`);
    assert.equal(agentOptions(a, "alpha").length, 1);
    assert.equal(agentOptions(a, "").length, 1);
  });
  it("suggerimenti dell'agente scelto", () => {
    const a = agentsOf([ALPHA, BETA]);
    assert.deepEqual(
      modelSuggestions(a, "alpha").map((m) => m.id),
      ["a-small", "a-big"],
    );
    assert.deepEqual(modelSuggestions(a, "ignoto"), []);
    assert.deepEqual(modelSuggestions(agentsOf([GAMMA, integration("x")]), "x"), []);
  });
  it("cambio agente: il modello resta solo se il nuovo agente lo consiglia", () => {
    const a = agentsOf([ALPHA, BETA]);
    assert.equal(modelAfterAgentChange(a, "alpha", "a-big"), "a-big");
    assert.equal(modelAfterAgentChange(a, "beta", "a-big"), "");
    assert.equal(modelAfterAgentChange(a, "beta", "  "), "");
    assert.equal(modelAfterAgentChange(a, "alpha", " a-small "), " a-small ");
    // Un agente senza suggerimenti non può garantire nessun nome.
    assert.equal(modelAfterAgentChange(agentsOf([integration("x")]), "x", "a-big"), "");
  });
  it("segnaposto del modello: worker del workspace, poi agente, poi generico", () => {
    const settings = { beta: { model: "b-ws", allow: [], deny: [] }, alpha: { model: null, allow: [], deny: [] } };
    const a = agentsOf([ALPHA, BETA], workspace({ worker_agents: settings }));
    assert.equal(modelPlaceholder(a, "beta"), "b-ws");
    assert.equal(modelPlaceholder(a, "alpha"), DEFAULT_MODEL_PLACEHOLDER);
    assert.equal(modelPlaceholder(agentsOf([BETA]), "beta"), "b-only");
    assert.equal(modelPlaceholder(a, "ignoto"), DEFAULT_MODEL_PLACEHOLDER);
  });
});

describe("agente nella patch del form", () => {
  const stored = {
    id: "t",
    title: "T",
    task: "S",
    depends_on: [],
    repo: null,
    agent: "alpha",
    model: "a-small",
    kind: "work",
  };
  it("il form parte dall'agente del task", () => {
    assert.equal(formFromTask(stored).agent, "alpha");
  });
  it("cambiare l'agente (e svuotare il modello) è nella patch", () => {
    const form = { ...formFromTask(stored), agent: "beta", model: "" };
    assert.deepEqual(buildPatch(stored, form, false), { agent: "beta", model: null });
  });
  it("agente invariato o non scelto: nessun campo agent", () => {
    assert.deepEqual(buildPatch(stored, formFromTask(stored), false), {});
    assert.deepEqual(buildPatch(stored, { ...formFromTask(stored), agent: "" }, false), {});
  });
});

describe("badge dell'agente", () => {
  it("con più integrazioni abilitate si mostra sempre", () => {
    const a = agentsOf([ALPHA, BETA]);
    assert.equal(agentBadge(a, "alpha"), "Agente alpha");
    assert.equal(agentBadge(a, "beta"), "Agente beta");
  });
  it("con una sola integrazione solo se differisce dal workspace", () => {
    const a = agentsOf([ALPHA]);
    assert.equal(agentBadge(a, "alpha"), null);
    assert.equal(agentBadge(a, "beta"), "beta");
    const own = agentsOf([ALPHA, OFF], workspace({ agent: "off" }));
    assert.equal(agentBadge(own, "off"), null);
    assert.equal(agentBadge(own, "alpha"), "Agente alpha");
  });
  it("niente prima del caricamento delle integrazioni o senza agente", () => {
    assert.equal(agentBadge(agentsOf([]), "alpha"), null);
    assert.equal(agentBadge(agentsOf([ALPHA, BETA]), ""), null);
  });
});

describe("proposte", () => {
  it("l'agente proposto, altrimenti quello del workspace, sempre mostrato", () => {
    const a = agentsOf([ALPHA]);
    assert.equal(proposalAgentLabel(a, "beta"), "beta");
    assert.equal(proposalAgentLabel(a, undefined), "Agente alpha");
    assert.equal(proposalAgentLabel(a, ""), "Agente alpha");
  });
  it("niente se le integrazioni non sono caricate o non c'è nessun agente", () => {
    assert.equal(proposalAgentLabel(agentsOf([]), "alpha"), null);
    assert.equal(proposalAgentLabel(agentsOf([integration("x")]), undefined), null);
  });
});

describe("filtro per agente", () => {
  const tasks = [{ agent: "b" }, { agent: "a" }, { agent: "b" }, { agent: "" }];
  it("agenti dei task senza doppioni, in ordine, più i selezionati", () => {
    assert.deepEqual(agentFilterOptions(tasks, []), ["a", "b"]);
    assert.deepEqual(agentFilterOptions(tasks, ["c", "a"]), ["a", "b", "c"]);
    assert.deepEqual(agentFilterOptions([], []), []);
  });
  it("il filtro si mostra con almeno due agenti", () => {
    assert.equal(showAgentFilter([]), false);
    assert.equal(showAgentFilter(["a"]), false);
    assert.equal(showAgentFilter(["a", "b"]), true);
  });
});

describe("agente non disponibile", () => {
  const a = agentsOf([ALPHA, OFF], workspace({ kind: "wsl", distro: "Ubuntu-24.04" }));
  it("testo: non pronto nell'ambiente del workspace", () => {
    for (const reason of ["not-found", "not-installed", "outdated"]) {
      assert.equal(
        unavailableText(a, { agent: "alpha", reason }),
        "In attesa: Agente alpha non è pronto in Ubuntu-24.04",
      );
    }
  });
  it("testo: disabilitato, senza ambiente", () => {
    assert.equal(unavailableText(a, { agent: "off", reason: "disabled" }), "In attesa: Agente off è disabilitato");
  });
  it("nessun testo se l'agente è pronto", () => {
    assert.equal(unavailableText(a, undefined), null);
  });
  it("dettaglio: cosa manca e come sbloccare", () => {
    assert.match(
      unavailableDetail(a, { agent: "alpha", reason: "not-found" }),
      /non è stato rilevato in Ubuntu-24\.04/,
    );
    assert.match(unavailableDetail(a, { agent: "alpha", reason: "not-installed" }), /pacchetto installato/);
    assert.match(unavailableDetail(a, { agent: "alpha", reason: "outdated" }), /da aggiornare/);
    assert.match(unavailableDetail(a, { agent: "off", reason: "disabled" }), /^Agente off è disabilitato\./);
    assert.match(unavailableDetail(a, { agent: "alpha", reason: "not-found" }), /parte da solo/);
  });
  it("agente sconosciuto: l'id", () => {
    assert.equal(
      unavailableText(a, { agent: "ignoto", reason: "not-found" }),
      "In attesa: ignoto non è pronto in Ubuntu-24.04",
    );
  });
});
