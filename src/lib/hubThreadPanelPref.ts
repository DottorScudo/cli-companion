// Agents HUB: il pannello task e terminali a destra del thread è mostrato o
// nascosto. Preferenza globale ricordata in localStorage. Funzioni pure a zero
// dipendenze, importabili sia dall'app sia dai test `node --test`: lo storage
// è iniettato.

import type { StorageLike } from "./taskPanelPrefs.ts";

/** Visibile di default. */
export const DEFAULT_THREAD_PANEL_SHOWN = true;

/** Chiave di localStorage: `"shown"` o `"hidden"`. */
export const THREAD_PANEL_STORAGE_KEY = "cli-companion.hub.thread-panel";

const SHOWN = "shown";
const HIDDEN = "hidden";

/** Un valore salvato che non è noto vale il default. */
export function parseThreadPanelShown(raw: unknown): boolean {
  if (raw === SHOWN) return true;
  if (raw === HIDDEN) return false;
  return DEFAULT_THREAD_PANEL_SHOWN;
}

/** Legge la preferenza; storage assente, vuoto o illeggibile = default. */
export function readThreadPanelShown(storage: StorageLike | null | undefined): boolean {
  try {
    return parseThreadPanelShown(storage?.getItem(THREAD_PANEL_STORAGE_KEY));
  } catch {
    return DEFAULT_THREAD_PANEL_SHOWN;
  }
}

/** Scrive la preferenza; gli errori di storage sono ignorati (vale solo in memoria). */
export function writeThreadPanelShown(storage: StorageLike | null | undefined, shown: boolean): void {
  try {
    storage?.setItem(THREAD_PANEL_STORAGE_KEY, shown ? SHOWN : HIDDEN);
  } catch {
    // Storage non disponibile (finestra privata, quota): la scelta vale fino alla chiusura.
  }
}
