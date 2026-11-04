// Agents HUB: salvataggio delle impostazioni (modello, API key, sessioni,
// lingua, segretari). Ogni comando del backend ritorna le impostazioni
// salvate, che finiscono nello store; rifiuta con l'errore del backend.

import { api } from "../../lib/tauri";
import type { BotImage } from "../../lib/bots";
import type { HubLanguage, HubSettingsView } from "../../lib/tauri";
import { errorMessage } from "../../lib/errors";
import type { SecretaryDraft } from "../../lib/hubSettings";
import { hubStore } from "../../stores/hub";

async function apply(saved: Promise<HubSettingsView>): Promise<void> {
  hubStore.settingsLoaded(await saved);
}

export const saveProvider = (baseUrl: string, model: string): Promise<void> =>
  apply(api.hubSetProvider(baseUrl, model));

/** La chiave passa solo da qui al backend (portachiavi del sistema): non resta in nessuno stato. */
export const saveApiKey = (key: string): Promise<void> => apply(api.hubSetApiKey(key));

export const clearApiKey = (): Promise<void> => apply(api.hubClearApiKey());

export const saveLanguage = (language: HubLanguage): Promise<void> => apply(api.hubSetLanguage(language));

export const saveSession = (idleMinutes: number, maxSteps: number): Promise<void> =>
  apply(api.hubSetSession(idleMinutes, maxSteps));

export const deleteSecretary = (name: string): Promise<void> => apply(api.hubDeleteSecretary(name));

/** Cosa fare dell'immagine di un segretario quando si salva. */
export type ImageChange = { kind: "keep" } | { kind: "set"; image: BotImage } | { kind: "remove" };

export interface SaveSecretaryResult {
  /** Il segretario è salvato ma l'immagine no: il motivo. */
  imageError: string | null;
}

/**
 * Crea (`original` `null`) o aggiorna il segretario, poi ne imposta o toglie
 * l'immagine. I campi rifiutano con l'errore del backend; un'immagine non
 * accettata non annulla il resto, che è già salvato.
 */
export async function saveSecretary(
  original: string | null,
  draft: SecretaryDraft,
  image: ImageChange,
): Promise<SaveSecretaryResult> {
  const tools = [...draft.tools];
  const channels = draft.channels === null ? null : [...draft.channels];
  if (original === null) {
    await apply(api.hubCreateSecretary(draft.name, draft.systemPrompt, tools, channels));
  } else {
    await apply(api.hubUpdateSecretary(original, draft.name, draft.systemPrompt, tools, channels));
  }
  try {
    if (image.kind === "set") await apply(api.hubSetSecretaryImage(draft.name, image.image.png, image.image.color));
    if (image.kind === "remove") await apply(api.hubRemoveSecretaryImage(draft.name));
  } catch (e) {
    return { imageError: errorMessage(e) };
  }
  return { imageError: null };
}
