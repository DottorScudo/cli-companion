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
