import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { effectiveWorkerAgentId, workerAgentSettings } from "../src/lib/workerAgent.ts";

const profiles = [
  { agent_id: "agent-a", is_default: true },
  { agent_id: "agent-b", is_default: false },
];

describe("effectiveWorkerAgentId", () => {
  it("l'agente del workspace vince sul predefinito globale", () => {
    assert.equal(effectiveWorkerAgentId({ agent: "agent-b" }, profiles), "agent-b");
  });

  it("senza agente nel workspace vale il predefinito globale", () => {
    assert.equal(effectiveWorkerAgentId({}, profiles), "agent-a");
    assert.equal(effectiveWorkerAgentId({ agent: undefined }, profiles), "agent-a");
  });

  it("un agente del workspace non nei profili resta quello scelto", () => {
    assert.equal(effectiveWorkerAgentId({ agent: "gone" }, profiles), "gone");
  });

  it("profili non ancora caricati: nessun agente", () => {
    assert.equal(effectiveWorkerAgentId({}, []), undefined);
  });
});

describe("workerAgentSettings", () => {
  const worker_agents = { "agent-a": { model: "m", allow: ["Read"], deny: [] } };

  it("legge la voce dell'agente", () => {
    assert.deepEqual(workerAgentSettings({ worker_agents }, "agent-a"), worker_agents["agent-a"]);
  });

  it("un agente senza voce, o nessun agente, ha impostazioni vuote", () => {
    const empty = { model: null, allow: [], deny: [] };
    assert.deepEqual(workerAgentSettings({ worker_agents }, "agent-b"), empty);
    assert.deepEqual(workerAgentSettings({ worker_agents }, undefined), empty);
  });
});
