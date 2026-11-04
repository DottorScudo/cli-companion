import { useEffect } from "react";
import { refreshAllSessionUsage, startLimitsPolling } from "../app/usage";
import { SESSION_POLL_MS, sessionPollInterval, usageTargets } from "../lib/usage";
import { configStore } from "../stores/config";
import { runtimeStore } from "../stores/tabRuntime";
import { tabsStore } from "../stores/tabs";
import { usageStore } from "../stores/usage";
import { useStore, useStores } from "./useStore";

// Chiave stabile delle coppie (agente, ambiente) con i limiti da leggere.
const targetSignature = (s: ReturnType<typeof configStore.getState>) =>
  usageTargets(s.agentProfiles, s.agentEnvs)
    .map((t) => t.key)
    .join("\n");

const WORKING_STORES = [tabsStore, runtimeStore] as const;

/** Almeno un tab in cui l'agente sta lavorando. */
const computeAnyWorking = () => {
  const runtime = runtimeStore.getState();
  return tabsStore.getState().some((t) => sessionPollInterval(runtime[t.key]?.agent?.phase) !== null);
};

/**
 * Polling dell'utilizzo dei coding agent, scritto in `usageStore`: limiti per
 * coppia (agente, ambiente) delle integrazioni che li dichiarano (all'avvio e
 * ogni 3 minuti) e token per sessione (a fine lavoro / richiesta di risposta /
 * avvio sessione, vedi `onAgentEventUsage`, e ogni 20 s mentre l'agente
 * lavora). Con `showUsage` falso non parte alcuna chiamata e lo stato è vuoto.
 */
export function useUsagePolling(): void {
  const enabled = useStore(configStore, (s) => s.showUsage);
  const targets = useStore(configStore, targetSignature);
  const anyWorking = useStores(WORKING_STORES, computeAnyWorking);

  useEffect(() => {
    if (!enabled || targets === "") {
      usageStore.clearLimits();
      return;
    }
    return startLimitsPolling();
  }, [enabled, targets]);

  useEffect(() => {
    if (!enabled) {
      usageStore.clearSessions();
      return;
    }
    // Alla riattivazione si rileggono i tab che hanno già un transcript.
    refreshAllSessionUsage(false);
  }, [enabled]);

  // Ogni 20 s per i tab in cui l'agente sta lavorando: il timer esiste solo
  // finché almeno un tab lavora.
  useEffect(() => {
    if (!enabled || !anyWorking) return;
    const id = setInterval(() => refreshAllSessionUsage(true), SESSION_POLL_MS);
    return () => clearInterval(id);
  }, [enabled, anyWorking]);

  // Tab chiusi: via i loro dati.
  useEffect(() => tabsStore.subscribe(() => usageStore.retainSessions(tabsStore.getState().map((t) => t.key))), []);
}
