// Utilizzo dei coding agent: lettura dei limiti per ambiente e dei token per
// sessione, scritti nell'archivio `usageStore`. Il timing (intervalli, quando
// partire) sta in `hooks/useUsage.ts`; qui le letture, usabili anche dagli
// eventi degli agenti.

import { api } from "../lib/tauri";
import { report } from "../lib/errors";
import {
  LIMITS_POLL_MS,
  sessionPollInterval,
  shouldRefreshSessionUsage,
  usageTargets,
  workspaceEnvKey,
} from "../lib/usage";
import { configStore } from "../stores/config";
import { runtimeStore } from "../stores/tabRuntime";
import { tabsStore } from "../stores/tabs";
import { usageStore } from "../stores/usage";
import { workspacesStore } from "../stores/workspaces";

/** Interruttore globale "Mostra utilizzo" delle Impostazioni. */
const usageEnabled = (): boolean => configStore.getState().showUsage;

/**
 * Legge i limiti di ogni coppia (agente, ambiente) che li dichiara, subito e
 * ogni `LIMITS_POLL_MS`. Restituisce la funzione che ferma il polling (e
 * scarta le risposte in volo).
 */
export function startLimitsPolling(): () => void {
  let cancelled = false;
  const load = () => {
    const { agentProfiles, agentEnvs } = configStore.getState();
    for (const { key, env } of usageTargets(agentProfiles, agentEnvs)) {
      api
        .agentUsageLimits(env.agent_id, env.env_key)
        .then((reply) => {
          if (!cancelled) usageStore.setLimit(key, reply);
        })
        .catch((e) => report("lettura dei limiti di utilizzo", e));
    }
  };
  load();
  const id = setInterval(load, LIMITS_POLL_MS);
  return () => {
    cancelled = true;
    clearInterval(id);
  };
}

/** Rilegge i token di una sessione dal suo transcript. */
export function refreshSessionUsage(
  key: string,
  transcriptOverride?: string | null,
  agentOverride?: string | null,
): void {
  if (!usageEnabled()) return;
  const tab = tabsStore.get(key);
  const agent = runtimeStore.get(key)?.agent;
  // Su session-start lo stato del tab non ha ancora id agente e transcript:
  // arrivano dall'evento.
  const agentId = agentOverride?.trim() || agent?.agentId;
  const transcript = transcriptOverride?.trim() || agent?.transcriptPath;
  const ws = tab && workspacesStore.get(tab.workspaceId);
  if (!tab || !ws || !agentId || !transcript) return;
  api
    .agentSessionUsage(agentId, workspaceEnvKey(ws), transcript)
    .then((usage) => {
      if (usageEnabled()) usageStore.setSession(key, usage);
    })
    .catch((e) => report("lettura dei token di sessione", e));
}

/**
 * Rilegge i token dei tab che hanno già un transcript: tutti (alla
 * riattivazione) o solo quelli in cui l'agente sta lavorando (ogni 20 s).
 */
export function refreshAllSessionUsage(onlyWorking: boolean): void {
  const runtime = runtimeStore.getState();
  for (const tab of tabsStore.getState()) {
    if (!onlyWorking || sessionPollInterval(runtime[tab.key]?.agent?.phase) !== null) refreshSessionUsage(tab.key);
  }
}

/** Da chiamare a ogni evento dell'agente di un tab. */
export function onAgentEventUsage(
  key: string,
  event: string,
  transcriptPath?: string | null,
  agentId?: string | null,
): void {
  if (shouldRefreshSessionUsage(event)) refreshSessionUsage(key, transcriptPath, agentId);
}
