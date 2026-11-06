// Menu degli avvii: quali voci mostrare per un workspace, con che etichetta e
// se l'integrazione è pronta nel suo ambiente. Logica pura a zero dipendenze:
// importabile dall'app e dai test `node --test`.

import type { BotLaunch } from "../bindings/BotLaunch.ts";
import type { BuiltinLaunch } from "../bindings/BuiltinLaunch.ts";
import type { IntegrationInfo } from "../bindings/IntegrationInfo.ts";
import type { Launch } from "../bindings/Launch.ts";
import type { Workspace } from "../bindings/Workspace.ts";
import { workspaceEnvKey } from "./usage.ts";

/** Separa il nome del preset da quello dell'agente: "Task Intake · Claude Code". */
const LABEL_SEPARATOR = " · ";

export interface LaunchMenuEntry {
  launch: BuiltinLaunch;
  label: string;
  /** Perché l'integrazione non è pronta nell'ambiente del workspace; `null` = si può avviare. */
  blockedReason: string | null;
}

/** Voci di un altro agente abilitato che dichiara i preset (sottomenu "Avvia con…"). */
export interface LaunchMenuAgent {
  agentId: string;
  agentName: string;
  entries: LaunchMenuEntry[];
}

/** Voce di un Bot agente. */
export interface LaunchMenuBotEntry {
  launch: BotLaunch;
  label: string;
  /** Il motivo del backend, altrimenti quello di prontezza dell'agente; `null` = si può avviare. */
  blockedReason: string | null;
}

export interface LaunchMenu {
  /** Un avvio per ogni integrazione abilitata, poi i preset con l'agente del workspace. */
  entries: LaunchMenuEntry[];
  /** Gli altri agenti che dichiarano i preset; vuoto = niente sottomenu. */
  others: LaunchMenuAgent[];
  /** I Bot agente, nell'ordine del backend. */
  bots: LaunchMenuBotEntry[];
}

/** Nome di un avvio come lo si legge in un elenco: i preset dicono con quale agente partono. */
export function launchLabel(launch: Launch | BuiltinLaunch): string {
  if ("preset" in launch && launch.preset !== null) return `${launch.name}${LABEL_SEPARATOR}${launch.agent_name}`;
  return launch.name;
}

/** La voce di un preset con l'agente scelto (`builtin:preset:<id>@<agent>`). */
function isChosenAgentPreset(launch: BuiltinLaunch): boolean {
  return launch.preset !== null && !launch.follows_workspace_agent;
}

/** Agente con cui partono i preset senza agente scelto, se si è potuto risolvere. */
function resolvedPresetAgent(launches: readonly BuiltinLaunch[]): string | undefined {
  return launches.find((l) => l.follows_workspace_agent)?.agent_id;
}

/**
 * Avvii da elencare in una scelta (pagine Avvii): senza le voci con l'agente
 * scelto uguale a quello già risolto, che sarebbero doppioni.
 */
export function distinctLaunches(launches: readonly BuiltinLaunch[]): BuiltinLaunch[] {
  const resolved = resolvedPresetAgent(launches);
  return launches.filter((l) => !(isChosenAgentPreset(l) && l.agent_id === resolved));
}

/** Perché `launch` non si può avviare nell'ambiente `envKey`; `null` se è pronto o lo stato non è noto. */
export function launchBlockReason(
  launch: Pick<BuiltinLaunch, "agent_id">,
  integrations: readonly IntegrationInfo[],
  envKey: string,
): string | null {
  const info = integrations.find((i) => i.id === launch.agent_id);
  const env = info?.envs.find((e) => e.status.env_key === envKey);
  if (!info || !env) return null;
  const where = env.status.env_label;
  switch (env.readiness) {
    case "not-found":
      return `${info.name} non è stato trovato in ${where}: installalo dalle Integrazioni.`;
    case "not-installed":
      return `Il pacchetto di ${info.name} non è installato in ${where}: installalo dalle Integrazioni.`;
    case "outdated":
      return `Il pacchetto di ${info.name} in ${where} è da aggiornare: aggiornalo dalle Integrazioni.`;
    case "ready":
    case "disabled":
      return null;
  }
}

/** Il menu degli avvii integrati di un workspace, da `launches` (già risolti per il suo agente). */
export function buildLaunchMenu(
  launches: readonly BuiltinLaunch[],
  integrations: readonly IntegrationInfo[],
  ws: Pick<Workspace, "kind" | "distro">,
  botLaunches: readonly BotLaunch[] = [],
): LaunchMenu {
  const envKey = workspaceEnvKey(ws);
  const entry = (launch: BuiltinLaunch, label: string): LaunchMenuEntry => ({
    launch,
    label,
    blockedReason: launchBlockReason(launch, integrations, envKey),
  });
  const entries = launches.filter((l) => !isChosenAgentPreset(l)).map((l) => entry(l, launchLabel(l)));
  const others: LaunchMenuAgent[] = [];
  for (const launch of distinctLaunches(launches).filter(isChosenAgentPreset)) {
    let agent = others.find((a) => a.agentId === launch.agent_id);
    if (!agent) {
      agent = { agentId: launch.agent_id, agentName: launch.agent_name, entries: [] };
      others.push(agent);
    }
    agent.entries.push(entry(launch, launch.name));
  }
  const bots = botLaunches.map((launch) => ({
    launch,
    label: launch.agent_name === null ? launch.name : `${launch.name}${LABEL_SEPARATOR}${launch.agent_name}`,
    blockedReason:
      launch.blocked_reason ??
      (launch.agent_id === null ? null : launchBlockReason({ agent_id: launch.agent_id }, integrations, envKey)),
  }));
  return { entries, others, bots };
}
