// L'agente dei worker di un workspace e le sue impostazioni (modello, regole di
// permesso). File a zero dipendenze: importabile sia dall'app sia dai test
// `node --test`.

import type { AgentProfile } from "../bindings/AgentProfile.ts";
import type { AgentWorkerSettings } from "../bindings/AgentWorkerSettings.ts";
import type { Workspace } from "../bindings/Workspace.ts";

/** Impostazioni di un agente che non ne ha: modello dell'agente, nessuna regola. */
const NO_SETTINGS: AgentWorkerSettings = { model: null, allow: [], deny: [] };

/**
 * Id dell'agente dei worker: quello scelto per il workspace, altrimenti il
 * predefinito globale (`undefined` finche' i profili non sono caricati).
 */
export function effectiveWorkerAgentId(
  workspace: Pick<Workspace, "agent">,
  profiles: readonly Pick<AgentProfile, "agent_id" | "is_default">[],
): string | undefined {
  return workspace.agent ?? profiles.find((p) => p.is_default)?.agent_id;
}

/** Impostazioni dei worker per l'agente `agentId` (vuote se non ne ha). */
export function workerAgentSettings(
  workspace: Pick<Workspace, "worker_agents">,
  agentId: string | undefined,
): AgentWorkerSettings {
  return (agentId !== undefined ? workspace.worker_agents[agentId] : undefined) ?? NO_SETTINGS;
}
