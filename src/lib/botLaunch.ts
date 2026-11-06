// Avvio di un Bot agente in un tab: pezzi puri a zero dipendenze, importabili
// dall'app e dai test `node --test`.

import type { AgentBot } from "../bindings/AgentBot.ts";
import type { BotLaunch } from "../bindings/BotLaunch.ts";
import type { BotTabLaunch } from "../bindings/BotTabLaunch.ts";

/** Separa gli avvisi dell'agente nel messaggio all'utente. */
const WARNING_SEPARATOR = "; ";

/** Chiave stabile dei campi dei Bot che cambiano gli avvii da elencare. */
export function agentBotsKey(agents: readonly AgentBot[]): string {
  return JSON.stringify(agents.map((a) => [a.id, a.name, a.enabled, a.agent, a.skill, a.model, a.effort]));
}

/** Avviso con ciò che l'agente non ha potuto applicare; `null` se non c'è nulla da dire. */
export function botWarningsText(launch: Pick<BotTabLaunch, "name" | "warnings">): string | null {
  if (launch.warnings.length === 0) return null;
  return `Bot «${launch.name}»: ${launch.warnings.join(WARNING_SEPARATOR)}`;
}

/** Il tab da aprire per un Bot: si salva il comando di ripristino, si esegue quello di avvio. */
export function botTabSpec(workspaceId: string, key: string, entry: Pick<BotLaunch, "id">, launch: BotTabLaunch) {
  return {
    key,
    workspaceId,
    launchId: entry.id,
    agentId: launch.agent_id,
    command: launch.restore_command,
    startCommand: launch.command,
  };
}

/** Prefisso degli id delle voci dei Bot (`bot:<id>`, vedi `bots/launches.rs`). */
export const BOT_LAUNCH_PREFIX = "bot:";

/** L'id del Bot di una voce di lancio; `undefined` se non è la voce di un Bot. */
export function launchBotId(launchId: string | null | undefined): string | undefined {
  return launchId?.startsWith(BOT_LAUNCH_PREFIX) ? launchId.slice(BOT_LAUNCH_PREFIX.length) : undefined;
}

/** La voce è quella di un Bot (non di un integrato o di un avvio del workspace)? */
export function isBotLaunch(launch: object): launch is BotLaunch {
  return "bot_id" in launch;
}

/** Un tab Bot ripristinato che chiede al backend l'ambiente del proprio Bot. */
export interface BotRestoreTarget {
  tabId: string;
  workspaceId: string;
  botId: string;
  /** L'agente con cui il tab era partito: il ripristino lo confronta con quello attuale del Bot. */
  agentId: string;
}

/** Dei tab ripristinati, quelli aperti da un Bot (la shell o un avvio di altro tipo non hanno nulla da rimettere). */
export function botRestoreTargets(
  tabs: readonly { key: string; workspaceId: string; launchId?: string; agentId?: string }[],
): BotRestoreTarget[] {
  return tabs.flatMap((tab) => {
    const botId = launchBotId(tab.launchId);
    if (botId === undefined || tab.agentId === undefined) return [];
    return [{ tabId: tab.key, workspaceId: tab.workspaceId, botId, agentId: tab.agentId }];
  });
}

/** Motivo mostrato quando il backend non risponde per un tab Bot. */
export const BOT_RESTORE_FAILED =
  "Non è stato possibile rimettere le impostazioni di un Bot nel terminale ripristinato.";

/** Un solo avviso per tutti i tab Bot ripristinati senza le loro scelte; `null` se non c'è nulla da dire. */
export function botRestoreNotice(warnings: readonly (string | null)[]): string | null {
  const unique = [...new Set(warnings.filter((w): w is string => w !== null))];
  return unique.length === 0 ? null : unique.join(" ");
}
