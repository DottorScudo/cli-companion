import { openHubWaiting } from "../app/actions/view";
import { useHubWaitingTotal } from "../hooks/useHubChannels";
import { useStore, useStores } from "../hooks/useStore";
import { useActiveWorld } from "../hooks/useView";
import { agentEnvBadge, installedAgentsSummary } from "../lib/agents";
import { limitGroups, usageTargets } from "../lib/usage";
import type { LimitGroup } from "../lib/usage";
import { configStore, enabledEnvs } from "../stores/config";
import { attentionCount } from "../stores/selectors";
import { runtimeStore } from "../stores/tabRuntime";
import { tabsStore } from "../stores/tabs";
import { usageStore } from "../stores/usage";
import HubStatusBar from "./HubStatusBar";
import StatusBar from "./StatusBar";

const ATTENTION_STORES = [tabsStore, runtimeStore] as const;
const LIMIT_STORES = [configStore, usageStore] as const;

const computeAttention = () => attentionCount(tabsStore.getState(), runtimeStore.getState());

/** Limiti di utilizzo per agente e ambiente (nessuno con l'interruttore spento). */
function computeLimits(): LimitGroup[] {
  const state = configStore.getState();
  if (!state.showUsage) return [];
  const { limits } = usageStore.getState();
  return limitGroups(
    usageTargets(state.agentProfiles, enabledEnvs(state)).map((t) => ({
      key: t.key,
      agentName: t.agentName,
      label: agentEnvBadge(t.env),
      reply: limits[t.key],
    })),
  );
}

/** Voci identiche per valore (le derivazioni creano oggetti nuovi a ogni chiamata). */
const sameLimits = (a: LimitGroup[], b: LimitGroup[]) => JSON.stringify(a) === JSON.stringify(b);

/**
 * Barra di stato: terminali aperti, in attesa di risposta, thread dell'Agents
 * HUB in attesa, limiti di utilizzo e plugin installati; nel mondo Agents HUB, la sua.
 */
export default function AppStatusBar() {
  const terminalCount = useStore(tabsStore, (s) => s.length);
  const attention = useStores(ATTENTION_STORES, computeAttention);
  const agentsSummary = useStore(configStore, (s) => installedAgentsSummary(enabledEnvs(s)));
  const limits = useStores(LIMIT_STORES, computeLimits, sameLimits);
  const hubWaiting = useHubWaitingTotal();
  const world = useActiveWorld();

  if (world === "hub") return <HubStatusBar />;
  return (
    <StatusBar
      terminalCount={terminalCount}
      attentionCount={attention}
      agentsSummary={agentsSummary}
      limits={limits}
      hubWaiting={hubWaiting}
      onOpenHub={openHubWaiting}
    />
  );
}
