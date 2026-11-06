// Menu degli avvii: quali voci mostrare per un workspace, con che etichetta e
// se l'integrazione è pronta nel suo ambiente. Logica pura a zero dipendenze:
// importabile dall'app e dai test `node --test`.

import type { BotLaunch } from "../bindings/BotLaunch.ts";
import type { BuiltinLaunch } from "../bindings/BuiltinLaunch.ts";
import type { IntegrationInfo } from "../bindings/IntegrationInfo.ts";
import type { Workspace } from "../bindings/Workspace.ts";
import { workspaceEnvKey } from "./usage.ts";

/** Separa il nome del Bot da quello del suo agente: "Intake · Claude Code". */
const LABEL_SEPARATOR = " · ";

export interface LaunchMenuEntry {
  launch: BuiltinLaunch;
  label: string;
  /** Perché l'integrazione non è pronta nell'ambiente del workspace; `null` = si può avviare. */
  blockedReason: string | null;
}

/** Voce di un Bot agente. */
export interface LaunchMenuBotEntry {
  launch: BotLaunch;
  label: string;
  /** Il motivo del backend, altrimenti quello di prontezza dell'agente; `null` = si può avviare. */
  blockedReason: string | null;
}

export interface LaunchMenu {
  /** Un avvio per ogni integrazione abilitata. */
  entries: LaunchMenuEntry[];
  /** I Bot agente, nell'ordine del backend. */
  bots: LaunchMenuBotEntry[];
}

/** Nome di un Bot con l'agente con cui parte, se si è potuto risolvere. */
export function botLaunchLabel(launch: Pick<BotLaunch, "name" | "agent_name">): string {
  return launch.agent_name === null ? launch.name : `${launch.name}${LABEL_SEPARATOR}${launch.agent_name}`;
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

/** Il menu degli avvii integrati e dei Bot di un workspace (`botLaunches` già risolti per il suo agente). */
export function buildLaunchMenu(
  launches: readonly BuiltinLaunch[],
  integrations: readonly IntegrationInfo[],
  ws: Pick<Workspace, "kind" | "distro">,
  botLaunches: readonly BotLaunch[] = [],
): LaunchMenu {
  const envKey = workspaceEnvKey(ws);
  const entries = launches.map((launch) => ({
    launch,
    label: launch.name,
    blockedReason: launchBlockReason(launch, integrations, envKey),
  }));
  const bots = botLaunches.map((launch) => ({
    launch,
    label: botLaunchLabel(launch),
    blockedReason:
      launch.blocked_reason ??
      (launch.agent_id === null ? null : launchBlockReason({ agent_id: launch.agent_id }, integrations, envKey)),
  }));
  return { entries, bots };
}
