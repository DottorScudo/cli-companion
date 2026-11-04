// Integrazioni con i coding agent: lettura dell'elenco nello store e azioni
// della pagina Impostazioni › Integrazioni. Le regole (almeno una abilitata,
// la predefinita non si disabilita) sono del backend: qui se ne mostra l'errore.

import { api } from "../../lib/tauri";
import type { IntegrationInfo } from "../../lib/tauri";
import { tabAgentId } from "../../lib/agents";
import { disableImpact, removableEnvs } from "../../lib/integrations";
import type { DisableImpact } from "../../lib/integrations";
import { errorMessage, report } from "../../lib/errors";
import { boardsStore } from "../../stores/boards";
import { configStore } from "../../stores/config";
import { runtimeStore } from "../../stores/tabRuntime";
import { tabsStore } from "../../stores/tabs";

let latestLoad = 0;

/**
 * Rilegge le integrazioni con lo stato di ogni ambiente (il backend sonda gli
 * ambienti). Una risposta superata da una richiesta più recente si scarta.
 */
export async function refreshIntegrations(): Promise<void> {
  const load = ++latestLoad;
  const integrations = await api.integrationsList();
  if (load === latestLoad) configStore.patch({ integrations });
}

/** Rilettura dopo un'operazione già riuscita: un suo errore non ne nasconde l'esito. */
async function refreshAfterChange(): Promise<void> {
  try {
    await refreshIntegrations();
  } catch (e) {
    report("rilevamento delle integrazioni", e);
  }
}

/** Terminali aperti e task in coda che dipendono dall'agente (le board non ancora caricate non contano). */
export function disableImpactOf(agentId: string): DisableImpact {
  const { agentProfiles } = configStore.getState();
  const runtime = runtimeStore.getState();
  const tabAgents = tabsStore
    .getState()
    .map((tab) => tabAgentId({ agent: runtime[tab.key]?.agent, startCommand: tab.startCommand }, agentProfiles));
  const tasks = Object.values(boardsStore.getState().boards).flatMap((board) => board.tasks);
  return disableImpact(agentId, tabAgents, tasks);
}

export async function setIntegrationEnabled(id: string, enabled: boolean): Promise<void> {
  await api.setIntegrationEnabled(id, enabled);
  await refreshAfterChange();
}

export async function setDefaultIntegration(id: string): Promise<void> {
  await api.setDefaultIntegration(id);
  await refreshAfterChange();
}

/** Installa o aggiorna il pacchetto in un ambiente; ritorna il log. */
export async function installPackage(agentId: string, envKey: string): Promise<string> {
  try {
    return await api.installAgentPlugin(agentId, envKey);
  } finally {
    await refreshAfterChange();
  }
}

/** Toglie il pacchetto da un ambiente; ritorna il log. */
export async function removePackage(agentId: string, envKey: string): Promise<string> {
  try {
    return await api.removeAgentPlugin(agentId, envKey);
  } finally {
    await refreshAfterChange();
  }
}

/**
 * Toglie il pacchetto da tutti gli ambienti in cui è presente, uno alla volta
 * (ogni operazione azzera la rilevazione): prova tutti e, se qualcuno fallisce,
 * rifiuta elencando gli errori per ambiente.
 */
export async function removeFromEnvironments(info: IntegrationInfo): Promise<string> {
  const logs: string[] = [];
  const failures: string[] = [];
  for (const { status } of removableEnvs(info)) {
    try {
      logs.push(await api.removeAgentPlugin(info.id, status.env_key));
    } catch (e) {
      failures.push(`${status.env_label}: ${errorMessage(e)}`);
    }
  }
  await refreshAfterChange();
  if (failures.length > 0) throw new Error(failures.join("\n"));
  return logs.join("\n");
}
