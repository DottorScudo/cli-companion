// Segretari predefiniti dell'Agents HUB (docs/HUB.MD): dal preset alla bozza
// dell'editor. I testi arrivano dal backend (`hub_list_secretary_presets`).
// Funzioni pure a zero dipendenze, importabili sia dall'app sia dai test.

import type { HubSecretary } from "../bindings/HubSecretary.ts";
import type { HubSecretaryPreset } from "../bindings/HubSecretaryPreset.ts";
import { checkSecretaryName } from "./hubSettings.ts";
import type { SecretaryDraft } from "./hubSettings.ts";

/** La bozza di un segretario nuovo creato dal preset: nome suggerito, prompt, tool, tutti i canali. */
export function draftOfPreset(preset: HubSecretaryPreset): SecretaryDraft {
  return { name: preset.name, systemPrompt: preset.system_prompt, tools: preset.tools, channels: null };
}

/** Il preset da cui nasce il segretario, se ne ha uno e questa versione lo conosce. */
export function presetOf(secretary: HubSecretary, presets: readonly HubSecretaryPreset[]): HubSecretaryPreset | null {
  return presets.find((p) => p.id === secretary.preset) ?? null;
}

/** Avviso per un segretario il cui preset ha un prompt diverso da quello salvato. */
export const PRESET_UPDATE_NOTICE = "È disponibile una versione aggiornata: Ripristina predefinito";

/**
 * Il preset da cui nasce il segretario, se il suo prompt ha ora un testo diverso
 * da quello salvato: c'è una versione aggiornata da ripristinare. Il confronto è
 * testuale, quindi anche un prompt personalizzato a mano la segnala.
 */
export function updatedPresetOf(
  secretary: HubSecretary,
  presets: readonly HubSecretaryPreset[],
): HubSecretaryPreset | null {
  const preset = presetOf(secretary, presets);
  return preset !== null && preset.system_prompt !== secretary.system_prompt ? preset : null;
}

/** Perché il nome suggerito dal preset non si può usare (già preso), `null` se è libero. */
export function presetNameError(preset: HubSecretaryPreset, taken: readonly string[]): string | null {
  const checked = checkSecretaryName(preset.name, taken);
  return checked.ok ? null : checked.reason;
}
