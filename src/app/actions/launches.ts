// Avvii integrati e dei Bot: lettura dal backend, uno per ogni agente proprio dei
// workspace oltre a quello senza agente (i preset partono con l'agente del workspace).

import { api } from "../../lib/tauri";
import { workspaceAgents } from "../../lib/launch";
import { configStore } from "../../stores/config";
import { workspacesStore } from "../../stores/workspaces";

let latestLoad = 0;

/** Avvii integrati e avvii dei Bot per i workspace con l'agente `agent`. */
function launchesOf(agent: string | null) {
  return Promise.all([api.builtinLaunches(agent), api.botLaunches(agent)]);
}

/**
 * Rilegge gli avvii integrati per ogni agente proprio dei workspace. Una
 * risposta superata da una richiesta più recente si scarta.
 */
export async function refreshLaunches(): Promise<void> {
  const load = ++latestLoad;
  const agents = workspaceAgents(workspacesStore.getState().list);
  const [[builtinLaunches, botLaunches], ...perAgent] = await Promise.all([
    launchesOf(null),
    ...agents.map((agent) => launchesOf(agent)),
  ]);
  if (load !== latestLoad) return;
  const agentLaunches = Object.fromEntries(agents.map((agent, i) => [agent, perAgent[i][0]]));
  const agentBotLaunches = Object.fromEntries(agents.map((agent, i) => [agent, perAgent[i][1]]));
  configStore.patch({ builtinLaunches, agentLaunches, botLaunches, agentBotLaunches });
}
