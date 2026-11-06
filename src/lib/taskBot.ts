// Il Bot di un task nell'interfaccia: scelta nei form, badge e modello da
// tenere cambiando Bot. Funzioni pure a zero dipendenze (solo i tipi dei
// bindings), importabili sia dall'app sia dai test `node --test`. Niente
// risoluzione di agente, modello, effort o skill: la fa il backend quando il
// task si salva (`board::agent::resolve_spec`), qui si mostra il risultato.

import type { AgentBot } from "../bindings/AgentBot.ts";

/** Il Bot per il lavoro senza specializzazione (stesso id dei preset del backend). */
export const GENERAL_BOT_ID = "general";

/** Voce del selettore per un task senza Bot (task vecchi). */
export const NO_BOT_LABEL = "Nessun Bot";

/** Indicazione accanto a un Bot che non si può più scegliere (disabilitato o cancellato). */
export const UNSELECTABLE_BOT_SUFFIX = "(non disponibile)";

/** Testo di un task senza Bot (task vecchi). */
export const NO_BOT_TEXT = "—";

export interface BotOption {
  id: string;
  label: string;
}

/** Il Bot `id`, se esiste ancora. */
export function botById(bots: readonly AgentBot[], id: string | undefined): AgentBot | undefined {
  return id ? bots.find((b) => b.id === id) : undefined;
}

/** Nome del Bot `id` (l'id stesso se è stato cancellato); `null` per un task senza Bot. */
export function botName(bots: readonly AgentBot[], id: string | undefined): string | null {
  if (!id) return null;
  return botById(bots, id)?.name ?? id;
}

/**
 * Voci del selettore: i Bot abilitati e, se `selected` non è fra questi (Bot
 * disabilitato o cancellato dopo la creazione del task), la voce corrente in
 * coda, così il campo mostra sempre un valore vero. "Nessun Bot" c'è solo per
 * chi non ne ha uno: ai task con Bot non si toglie.
 */
export function botOptions(bots: readonly AgentBot[], selected: string): BotOption[] {
  const options = bots.filter((b) => b.enabled).map((b) => ({ id: b.id, label: b.name }));
  if (selected === "") return [{ id: "", label: NO_BOT_LABEL }, ...options];
  if (!options.some((o) => o.id === selected)) {
    options.push({ id: selected, label: `${botName(bots, selected)} ${UNSELECTABLE_BOT_SUFFIX}` });
  }
  return options;
}

/** Bot preselezionato in un task nuovo: `general` se abilitato, altrimenti il primo abilitato; vuoto se non ce ne sono. */
export function defaultBotChoice(bots: readonly AgentBot[]): string {
  const enabled = bots.filter((b) => b.enabled);
  return (enabled.find((b) => b.id === GENERAL_BOT_ID) ?? enabled[0])?.id ?? "";
}

/**
 * Agente del Bot `id` per i suggerimenti del modello: il suo, altrimenti quello
 * del workspace. Vuoto se il Bot non c'è.
 */
export function botAgentId(bots: readonly AgentBot[], id: string, workspaceAgentId: string | undefined): string {
  const bot = botById(bots, id);
  return bot ? (bot.agent ?? workspaceAgentId ?? "") : "";
}
