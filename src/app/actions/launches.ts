// Avvii integrati: lettura dal backend, uno per ogni agente proprio dei
// workspace oltre a quello senza agente (i preset partono con l'agente del workspace).

import { api } from "../../lib/tauri";
import { workspaceAgents } from "../../lib/launch";
import { configStore } from "../../stores/config";
import { workspacesStore } from "../../stores/workspaces";

let latestLoad = 0;

/**
 * Rilegge gli avvii integrati per ogni agente proprio dei workspace. Una
 * risposta superata da una richiesta più recente si scarta.
 */
export async function refreshLaunches(): Promise<void> {
  const load = ++latestLoad;
  const agents = workspaceAgents(workspacesStore.getState().list);
  const [builtinLaunches, ...perAgent] = await Promise.all([
    api.builtinLaunches(null),
    ...agents.map((agent) => api.builtinLaunches(agent)),
  ]);
  if (load !== latestLoad) return;
  const agentLaunches = Object.fromEntries(agents.map((agent, i) => [agent, perAgent[i]]));
  configStore.patch({ builtinLaunches, agentLaunches });
}
