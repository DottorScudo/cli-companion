// Cosa è mostrato al posto dei terminali: Tasks Manager del workspace in
// vista e Ufficio. Funzioni pure sullo stato degli archivi, condivise da
// hook (`hooks/useView.ts`) e azioni (`app/actions/*`).

import type { BotSettings } from "../lib/tauri.ts";
import { viewWorkspaceId } from "./layout.ts";
import type { LayoutStoreState } from "./layout.ts";
import type { UiState } from "./ui.ts";

/** L'Ufficio è globale (non per workspace) e vive solo con i bot attivi. */
export function officeShown(ui: Pick<UiState, "officeOpen">, bots: Pick<BotSettings, "enabled">): boolean {
  return ui.officeOpen && bots.enabled;
}

/**
 * Il Tasks Manager copre l'area del pannello Task e dei terminali del
 * workspace mostrato (mai nella vista Globale).
 */
export function managerShown(layout: LayoutStoreState, ui: Pick<UiState, "managerOpen">): boolean {
  const id = viewWorkspaceId(layout);
  return id !== null && ui.managerOpen[id] === true;
}

/** Manager o Ufficio coprono pannello Task e terminali. */
export function overlayShown(
  layout: LayoutStoreState,
  ui: Pick<UiState, "managerOpen" | "officeOpen">,
  bots: Pick<BotSettings, "enabled">,
): boolean {
  return managerShown(layout, ui) || officeShown(ui, bots);
}
