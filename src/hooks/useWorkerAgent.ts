import { configStore } from "../stores/config";
import { effectiveWorkerAgentId, workerAgentSettings } from "../lib/workerAgent";
import type { Workspace } from "../lib/tauri";
import { useStore } from "./useStore";

/**
 * L'agente dei worker del workspace (il suo, altrimenti il predefinito globale),
 * il suo profilo e le impostazioni dei worker che ha nel workspace.
 */
export function useWorkerAgent(workspace: Workspace) {
  const profiles = useStore(configStore, (s) => s.agentProfiles);
  const agentId = effectiveWorkerAgentId(workspace, profiles);
  return {
    agentId,
    profile: profiles.find((p) => p.agent_id === agentId),
    settings: workerAgentSettings(workspace, agentId),
  };
}
