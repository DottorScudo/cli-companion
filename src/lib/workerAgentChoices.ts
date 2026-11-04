// Scelte della pagina Worker: quale agente esegue i worker del workspace, quali
// modelli suggerisce e quali altri agenti hanno impostazioni salvate. Logica
// pura a zero dipendenze (nessun nome di agente: tutto viene dai contratti
// dichiarati dalle integrazioni), importabile dall'app e dai test `node --test`.

import type { AgentProfile } from "../bindings/AgentProfile.ts";
import type { AgentWorkerSettings } from "../bindings/AgentWorkerSettings.ts";
import type { IntegrationInfo } from "../bindings/IntegrationInfo.ts";
import type { ModelTier } from "../bindings/ModelTier.ts";
import { defaultOf, enabledOf } from "./integrations.ts";
import { workerAgentSettings } from "./workerAgent.ts";

/** Valore dell'opzione "Eredita": nessun agente nel workspace, vale il predefinito globale. */
export const INHERIT_AGENT_VALUE = "";

/** Testo del campo modello vuoto, quando l'integrazione non indica un modello predefinito. */
export const MODEL_DEFAULT_PLACEHOLDER = "predefinito dell’agente";

/** Etichetta italiana di ogni fascia: la tabella è esaustiva, una fascia nuova in Rust non compila. */
const TIER_LABELS: Record<ModelTier, string> = {
  standard: "standard",
  strong: "avanzato",
};

export interface AgentOption {
  value: string;
  label: string;
}

export interface ModelSuggestion {
  /** Nome come lo vuole l'agente. */
  id: string;
  /** Nome e fascia, per l'utente. */
  label: string;
}

export interface ConfiguredAgent {
  id: string;
  name: string;
}

type CanRunWorkers = Pick<IntegrationInfo, "capabilities">;

/** `true` se l'integrazione dichiara l'automazione, cioè sa eseguire i worker. */
export function canRunWorkers(info: CanRunWorkers): boolean {
  return info.capabilities.includes("automation");
}

/** Integrazioni a cui si può affidare i worker: abilitate e con l'automazione. */
function workerCapable(integrations: readonly IntegrationInfo[]): IntegrationInfo[] {
  return enabledOf(integrations).filter(canRunWorkers);
}

function agentName(id: string, integrations: readonly IntegrationInfo[]): string {
  return integrations.find((i) => i.id === id)?.name ?? id;
}

/** Testo dell'opzione "Eredita", col nome del predefinito globale se noto. */
export function inheritAgentLabel(integrations: readonly IntegrationInfo[]): string {
  const fallback = defaultOf(integrations);
  return fallback ? `Eredita (${fallback.name})` : "Eredita";
}

/**
 * Perché l'agente salvato nel workspace non può eseguire i worker, o `null`
 * se può (o se il workspace eredita). Il testo è l'avviso mostrato nel campo.
 */
export function workerAgentWarning(
  agentId: string | undefined,
  integrations: readonly IntegrationInfo[],
): string | null {
  if (agentId === undefined) return null;
  const info = integrations.find((i) => i.id === agentId);
  if (!info) return `L’agente «${agentId}» non è più disponibile: scegline un altro.`;
  if (!info.enabled) {
    return `${info.name} è disabilitato: i nuovi task non partono finché non lo riabiliti o scegli un altro agente.`;
  }
  if (!canRunWorkers(info)) return `${info.name} non supporta i worker automatici: scegli un altro agente.`;
  return null;
}

/**
 * Opzioni del campo "Agente dei worker": Eredita, poi le integrazioni abilitate
 * con l'automazione. L'agente salvato ma non più utilizzabile resta in elenco
 * (segnato) per mostrare il valore reale invece di un'altra scelta.
 */
export function workerAgentOptions(
  integrations: readonly IntegrationInfo[],
  workspaceAgent: string | undefined,
): AgentOption[] {
  const options: AgentOption[] = [
    { value: INHERIT_AGENT_VALUE, label: inheritAgentLabel(integrations) },
    ...workerCapable(integrations).map((i) => ({ value: i.id, label: i.name })),
  ];
  if (workspaceAgent !== undefined && workerAgentWarning(workspaceAgent, integrations) !== null) {
    options.push({ value: workspaceAgent, label: `${agentName(workspaceAgent, integrations)} (non utilizzabile)` });
  }
  return options;
}

/** Modelli suggeriti dall'integrazione, con la fascia nell'etichetta. */
export function modelSuggestions(info: Pick<IntegrationInfo, "models"> | undefined): ModelSuggestion[] {
  return (info?.models ?? []).map((m) => ({ id: m.id, label: `${m.label} · ${TIER_LABELS[m.tier]}` }));
}

/** Segnaposto del modello vuoto: dice qual è il modello predefinito dell'agente, se l'integrazione lo indica. */
export function modelPlaceholder(info: Pick<IntegrationInfo, "models" | "default_model"> | undefined): string {
  if (!info?.default_model) return MODEL_DEFAULT_PLACEHOLDER;
  const label = info.models.find((m) => m.id === info.default_model)?.label ?? info.default_model;
  return `${MODEL_DEFAULT_PLACEHOLDER} (${label})`;
}

const hasSettings = (s: AgentWorkerSettings): boolean => s.model !== null || s.allow.length > 0 || s.deny.length > 0;

/**
 * Gli altri agenti utilizzabili con impostazioni salvate nel workspace (modello
 * o regole), nell'ordine dei profili: permettono di passare dall'uno all'altro.
 */
export function otherConfiguredAgents(
  workerAgents: Readonly<Record<string, AgentWorkerSettings>>,
  shownAgentId: string | undefined,
  profiles: readonly Pick<AgentProfile, "agent_id" | "agent_name">[],
): ConfiguredAgent[] {
  return profiles
    .filter(
      (p) =>
        p.agent_id !== shownAgentId && hasSettings(workerAgentSettings({ worker_agents: workerAgents }, p.agent_id)),
    )
    .map((p) => ({ id: p.agent_id, name: p.agent_name }));
}
