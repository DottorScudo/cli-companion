// L'agente di un task nell'interfaccia: scelta nei form, badge, filtro e motivo
// per cui un task in coda non parte. Funzioni pure a zero dipendenze (solo i
// tipi dei bindings e integrations.ts), importabili sia dall'app sia dai test
// `node --test`. Nessun nome di agente o di modello: tutto viene dai dati delle
// integrazioni.

import { defaultOf, enabledOf } from "./integrations.ts";
import type { AgentWorkerSettings } from "../bindings/AgentWorkerSettings.ts";
import type { AgentUnavailableView } from "../bindings/AgentUnavailableView.ts";
import type { IntegrationInfo } from "../bindings/IntegrationInfo.ts";
import type { Readiness } from "../bindings/Readiness.ts";
import type { SuggestedModel } from "../bindings/SuggestedModel.ts";
import type { Workspace } from "../bindings/Workspace.ts";

/** Segnaposto del campo modello quando né il workspace né l'agente ne indicano uno. */
export const DEFAULT_MODEL_PLACEHOLDER = "predefinito dei worker";

/** Indicazione accanto all'agente del workspace nel selettore. */
export const WORKSPACE_AGENT_SUFFIX = "(predefinito del workspace)";

/** Indicazione accanto a un agente che non si può più scegliere (disabilitato o senza worker). */
export const UNSELECTABLE_AGENT_SUFFIX = "(non disponibile)";

/** Ambiente di un workspace Windows nel testo "non è pronto in …". */
const WINDOWS_ENV_LABEL = "Windows";

/** Perché un agente non è pronto, in italiano: una riga per ogni stato di prontezza. */
const READINESS_DETAIL: Record<Readiness, string> = {
  ready: "è pronto",
  "not-found": "non è stato rilevato",
  "not-installed": "non ha il pacchetto installato",
  outdated: "ha il pacchetto da aggiornare",
  disabled: "è disabilitato",
};

/** Dati di un workspace e delle integrazioni che servono agli agenti dei task. */
export interface TaskAgents {
  /** Agenti selezionabili per un task: abilitati e con i worker (`automation`). */
  choices: readonly IntegrationInfo[];
  /** Agente del workspace: il suo, altrimenti il predefinito globale. */
  workspaceAgentId: string | undefined;
  /** Nel workspace c'è più di un'integrazione abilitata. */
  multiple: boolean;
  /** Le integrazioni sono state caricate. */
  ready: boolean;
  /** Nome mostrato di ogni integrazione registrata, abilitata o no. */
  names: ReadonlyMap<string, string>;
  /** Modelli e regole dei worker del workspace, per agente. */
  workerSettings: Readonly<Record<string, AgentWorkerSettings>>;
  /** Ambiente in cui il workspace esegue i task ("Windows" o la distro WSL). */
  envLabel: string;
}

type WorkspaceAgentFields = Pick<Workspace, "agent" | "kind" | "distro" | "worker_agents">;

/** Ambiente del workspace per il testo dell'agente non pronto. */
export function workspaceEnvLabel(workspace: Pick<Workspace, "kind" | "distro">): string {
  return workspace.kind === "wsl" && workspace.distro ? workspace.distro : WINDOWS_ENV_LABEL;
}

/** Costruisce i dati degli agenti dei task di `workspace` dalle integrazioni registrate. */
export function taskAgents(integrations: readonly IntegrationInfo[], workspace: WorkspaceAgentFields): TaskAgents {
  return {
    choices: enabledOf(integrations).filter((i) => i.capabilities.includes("automation")),
    workspaceAgentId: workspace.agent ?? defaultOf(integrations)?.id,
    multiple: enabledOf(integrations).length > 1,
    ready: integrations.length > 0,
    names: new Map(integrations.map((i) => [i.id, i.name])),
    workerSettings: workspace.worker_agents,
    envLabel: workspaceEnvLabel(workspace),
  };
}

/** Nome dell'agente `id` (l'id stesso se non è fra le integrazioni registrate). */
export function agentName(agents: Pick<TaskAgents, "names">, id: string): string {
  return agents.names.get(id) ?? id;
}

// ---------------------------------------------------------------------------
// Scelta nei form
// ---------------------------------------------------------------------------

export interface AgentOption {
  id: string;
  label: string;
}

/** Agente preselezionato in un task nuovo: quello del workspace se si può scegliere, altrimenti il primo; vuoto se non c'è scelta. */
export function defaultAgentChoice(agents: Pick<TaskAgents, "choices" | "workspaceAgentId">): string {
  const preferred = agents.choices.find((i) => i.id === agents.workspaceAgentId);
  return (preferred ?? agents.choices[0])?.id ?? "";
}

/**
 * Voci del selettore: gli agenti selezionabili (quello del workspace marcato) e,
 * se `selected` non è fra questi (task il cui agente è stato disabilitato), la
 * voce corrente in coda, così il campo mostra sempre un valore vero.
 */
export function agentOptions(
  agents: Pick<TaskAgents, "choices" | "workspaceAgentId" | "names">,
  selected: string,
): AgentOption[] {
  const options = agents.choices.map((i) => ({
    id: i.id,
    label: i.id === agents.workspaceAgentId ? `${i.name} ${WORKSPACE_AGENT_SUFFIX}` : i.name,
  }));
  if (selected !== "" && !options.some((o) => o.id === selected)) {
    options.push({ id: selected, label: `${agentName(agents, selected)} ${UNSELECTABLE_AGENT_SUFFIX}` });
  }
  return options;
}

/** Modelli consigliati dall'agente `id` (vuoti se non li dichiara). */
export function modelSuggestions(agents: Pick<TaskAgents, "choices">, id: string): readonly SuggestedModel[] {
  return agents.choices.find((i) => i.id === id)?.models ?? [];
}

/**
 * Modello da tenere cambiando agente: quello scritto resta solo se il nuovo
 * agente lo consiglia (i nomi dei modelli sono dell'agente); altrimenti si svuota.
 */
export function modelAfterAgentChange(agents: Pick<TaskAgents, "choices">, agentId: string, model: string): string {
  const name = model.trim();
  if (name === "") return "";
  return modelSuggestions(agents, agentId).some((m) => m.id === name) ? model : "";
}

/** Segnaposto del campo modello: quello dei worker del workspace per l'agente, poi il predefinito dell'agente. */
export function modelPlaceholder(agents: Pick<TaskAgents, "choices" | "workerSettings">, agentId: string): string {
  return (
    agents.workerSettings[agentId]?.model ||
    agents.choices.find((i) => i.id === agentId)?.default_model ||
    DEFAULT_MODEL_PLACEHOLDER
  );
}

// ---------------------------------------------------------------------------
// Badge
// ---------------------------------------------------------------------------

/**
 * Nome dell'agente da mostrare su un task, o `null` per non fare rumore: con
 * una sola integrazione abilitata si mostra solo se l'agente non è quello del
 * workspace. Niente prima che le integrazioni siano caricate.
 */
export function agentBadge(agents: TaskAgents, taskAgent: string): string | null {
  if (!agents.ready || taskAgent === "") return null;
  if (!agents.multiple && taskAgent === agents.workspaceAgentId) return null;
  return agentName(agents, taskAgent);
}

/** Nome dell'agente di una specifica proposta (assente = quello del workspace): sempre mostrato, se noto. */
export function proposalAgentLabel(agents: TaskAgents, specAgent: string | undefined): string | null {
  const id = specAgent || agents.workspaceAgentId;
  return agents.ready && id ? agentName(agents, id) : null;
}

// ---------------------------------------------------------------------------
// Filtro
// ---------------------------------------------------------------------------

/** Agenti dei task, senza doppioni e in ordine alfabetico, più quelli già selezionati nel filtro (restano deselezionabili). */
export function agentFilterOptions(tasks: readonly { agent: string }[], selected: readonly string[]): string[] {
  const ids = new Set<string>(selected);
  for (const t of tasks) if (t.agent !== "") ids.add(t.agent);
  return [...ids].sort((a, b) => a.localeCompare(b));
}

/** Il filtro per agente ha senso solo con almeno due agenti fra cui scegliere. */
export function showAgentFilter(options: readonly string[]): boolean {
  return options.length > 1;
}

// ---------------------------------------------------------------------------
// Agente non disponibile
// ---------------------------------------------------------------------------

/** Testo sintetico dello stato: "In attesa: X non è pronto in Windows" (o "è disabilitato"). */
export function unavailableText(agents: TaskAgents, unavailable: AgentUnavailableView | undefined): string | null {
  if (!unavailable) return null;
  const name = agentName(agents, unavailable.agent);
  return unavailable.reason === "disabled"
    ? `In attesa: ${name} è disabilitato`
    : `In attesa: ${name} non è pronto in ${agents.envLabel}`;
}

/** Spiegazione estesa per il dettaglio: cosa manca e che il task parte da solo appena l'agente è pronto. */
export function unavailableDetail(agents: TaskAgents, unavailable: AgentUnavailableView): string {
  const name = agentName(agents, unavailable.agent);
  const why =
    unavailable.reason === "disabled"
      ? `${name} è disabilitato`
      : `${name} ${READINESS_DETAIL[unavailable.reason]} in ${agents.envLabel}`;
  return `${why}. Il task parte da solo appena l’agente è pronto, oppure cambia agente.`;
}
