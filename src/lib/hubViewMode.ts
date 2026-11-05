// Agents HUB (docs/HUB.MD › Interfaccia): come si mostrano i thread, per canale
// o come messenger (chat di tutti i canali a sinistra, conversazione a destra).
// Preferenza globale ricordata in localStorage. Funzioni pure a zero dipendenze,
// importabili sia dall'app sia dai test `node --test`: lo storage è iniettato.

import type { StorageLike } from "./taskPanelPrefs.ts";

export type HubViewMode = "channels" | "messenger";

export const DEFAULT_HUB_VIEW_MODE: HubViewMode = "channels";

/** Chiave di localStorage: `"channels"` o `"messenger"`. */
export const HUB_VIEW_MODE_STORAGE_KEY = "cli-companion.hub.view-mode";

/** Un valore salvato che non è una modalità nota vale la modalità di default. */
export function parseViewMode(raw: unknown): HubViewMode {
  return raw === "messenger" || raw === "channels" ? raw : DEFAULT_HUB_VIEW_MODE;
}

/** Legge la modalità; storage assente, vuoto o illeggibile = per canale. */
export function readViewMode(storage: StorageLike | null | undefined): HubViewMode {
  try {
    return parseViewMode(storage?.getItem(HUB_VIEW_MODE_STORAGE_KEY));
  } catch {
    return DEFAULT_HUB_VIEW_MODE;
  }
}

/** Scrive la modalità; gli errori di storage sono ignorati (vale solo in memoria). */
export function writeViewMode(storage: StorageLike | null | undefined, mode: HubViewMode): void {
  try {
    storage?.setItem(HUB_VIEW_MODE_STORAGE_KEY, mode);
  } catch {
    // Storage non disponibile (finestra privata, quota): la scelta vale fino alla chiusura.
  }
}
