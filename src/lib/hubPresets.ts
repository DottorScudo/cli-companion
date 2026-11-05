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

/** Perché il nome suggerito dal preset non si può usare (già preso), `null` se è libero. */
export function presetNameError(preset: HubSecretaryPreset, taken: readonly string[]): string | null {
  const checked = checkSecretaryName(preset.name, taken);
  return checked.ok ? null : checked.reason;
}
