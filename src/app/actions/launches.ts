// Avvii integrati e dei Bot: lettura dal backend. Gli integrati sono uno per
// agente abilitato; i Bot si leggono per ogni agente proprio dei workspace oltre
// a quello senza agente (un Bot senza agente fissato parte con quello del workspace).

import { api } from "../../lib/tauri";
import { workspaceAgents } from "../../lib/launch";
import { configStore } from "../../stores/config";
import { workspacesStore } from "../../stores/workspaces";

let latestLoad = 0;

/**
 * Rilegge gli avvii integrati e quelli dei Bot per ogni agente proprio dei
 * workspace. Una risposta superata da una richiesta più recente si scarta.
 */
export async function refreshLaunches(): Promise<void> {
  const load = ++latestLoad;
  const agents = workspaceAgents(workspacesStore.getState().list);
  const [builtinLaunches, botLaunches, ...perAgent] = await Promise.all([
    api.builtinLaunches(),
    api.botLaunches(null),
    ...agents.map((agent) => api.botLaunches(agent)),
  ]);
  if (load !== latestLoad) return;
  const agentBotLaunches = Object.fromEntries(agents.map((agent, i) => [agent, perAgent[i]]));
  configStore.patch({ builtinLaunches, botLaunches, agentBotLaunches });
}
