// Agents HUB: suggerimento a creare l'Operatore (preset `operator`), che non è
// seeded perché può cambiare la board (docs/POOL.MD §3.4, decisione D3). Si
// mostra finché non c'è un segretario nato da quel preset e l'utente non lo
// chiude; la chiusura è ricordata in localStorage. Funzioni pure a zero
// dipendenze, importabili sia dall'app sia dai test `node --test`: lo storage
// è iniettato.

import type { HubSecretary } from "../bindings/HubSecretary.ts";
import type { HubSecretaryPreset } from "../bindings/HubSecretaryPreset.ts";
import type { StorageLike } from "./taskPanelPrefs.ts";

/** Id del preset dell'Operatore nel backend (`hub/settings/presets.rs`). */
export const OPERATOR_PRESET_ID = "operator";

/** Chiave di localStorage: `"dismissed"` dopo la chiusura del suggerimento. */
export const OPERATOR_HINT_STORAGE_KEY = "cli-companion.hub.operator-hint";

const DISMISSED = "dismissed";

/**
 * Il preset da suggerire, o `null` se il suggerimento non va mostrato: chiuso
 * dall'utente, Operatore già presente (anche rinominato) o preset sconosciuto
 * a questa versione.
 */
export function operatorHintPreset(
  secretaries: readonly HubSecretary[],
  presets: readonly HubSecretaryPreset[],
  dismissed: boolean,
): HubSecretaryPreset | null {
  if (dismissed || secretaries.some((s) => s.preset === OPERATOR_PRESET_ID)) return null;
  return presets.find((p) => p.id === OPERATOR_PRESET_ID) ?? null;
}

/** Legge la chiusura; storage assente, vuoto o illeggibile = non chiuso. */
export function readOperatorHintDismissed(storage: StorageLike | null | undefined): boolean {
  try {
    return storage?.getItem(OPERATOR_HINT_STORAGE_KEY) === DISMISSED;
  } catch {
    return false;
  }
}

/** Ricorda la chiusura; gli errori di storage sono ignorati (vale solo in memoria). */
export function writeOperatorHintDismissed(storage: StorageLike | null | undefined): void {
  try {
    storage?.setItem(OPERATOR_HINT_STORAGE_KEY, DISMISSED);
  } catch {
    // Storage non disponibile (finestra privata, quota): la chiusura vale fino alla chiusura dell'app.
  }
}
