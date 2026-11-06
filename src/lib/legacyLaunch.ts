// Le vecchie voci di lancio dei preset (prima dei Bot), che restano nelle
// sessioni salvate da una versione precedente: la loro corrispondenza con i Bot.
// Congelata come in `src-tauri/src/launch_migration/ids.rs`. Zero dipendenze:
// importabile dall'app e dai test `node --test`.

import { BOT_LAUNCH_PREFIX } from "./botLaunch.ts";

const LEGACY_ORCHESTRATOR_LAUNCH = "builtin:orchestrator";
const LEGACY_PRESET_PREFIX = "builtin:preset:";
const LEGACY_AGENT_SEPARATOR = "@";

/** Il preset di allora -> il Bot predefinito che lo sostituisce. */
const PRESET_BOTS: ReadonlyMap<string, string> = new Map([
  ["task-intake", "intake"],
  ["project-setup", "setup"],
  ["skill-wizard", "skill-wizard"],
]);

/** Il Bot dell'orchestratore, come quello del preset Task Intake. */
const ORCHESTRATOR_BOT = "intake";

/** Id di un agente: minuscole, cifre e `-`, al massimo 64 caratteri. */
const AGENT_ID = /^[a-z0-9-]{1,64}$/;

/** La voce del Bot di una vecchia voce di lancio (derivato se c'era l'agente); gli altri id restano. */
export function currentLaunchId(id: string): string {
  if (id === LEGACY_ORCHESTRATOR_LAUNCH) return `${BOT_LAUNCH_PREFIX}${ORCHESTRATOR_BOT}`;
  if (!id.startsWith(LEGACY_PRESET_PREFIX)) return id;
  const rest = id.slice(LEGACY_PRESET_PREFIX.length);
  const at = rest.indexOf(LEGACY_AGENT_SEPARATOR);
  const preset = at < 0 ? rest : rest.slice(0, at);
  const agent = at < 0 ? undefined : rest.slice(at + 1);
  const bot = PRESET_BOTS.get(preset);
  if (bot === undefined) return id;
  return agent !== undefined && AGENT_ID.test(agent)
    ? `${BOT_LAUNCH_PREFIX}${bot}-${agent}`
    : `${BOT_LAUNCH_PREFIX}${bot}`;
}
