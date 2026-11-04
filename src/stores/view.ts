// Cosa è mostrato al posto dei terminali: Tasks Manager del workspace in
// vista, Ufficio e Agents HUB. Funzioni pure sullo stato degli archivi, condivise da
// hook (`hooks/useView.ts`) e azioni (`app/actions/*`).

import type { BotSettings } from "../lib/tauri.ts";
import { viewWorkspaceId } from "./layout.ts";
import type { LayoutStoreState } from "./layout.ts";
import type { HubState } from "./hub.ts";
import type { UiState } from "./ui.ts";

/** L'Ufficio è globale (non per workspace) e vive solo con i bot attivi. */
export function officeShown(ui: Pick<UiState, "officeOpen">, bots: Pick<BotSettings, "enabled">): boolean {
  return ui.officeOpen && bots.enabled;
}

/** L'Agents HUB è globale e vive solo con l'interruttore acceso. */
export function hubShown(ui: Pick<UiState, "hubOpen">, hub: Pick<HubState, "enabled">): boolean {
  return ui.hubOpen && hub.enabled;
}

/**
 * Il Tasks Manager copre l'area del pannello Task e dei terminali del
 * workspace mostrato (mai nella vista Globale).
 */
export function managerShown(layout: LayoutStoreState, ui: Pick<UiState, "managerOpen">): boolean {
  const id = viewWorkspaceId(layout);
  return id !== null && ui.managerOpen[id] === true;
}

/** Manager, Ufficio o Agents HUB coprono pannello Task e terminali. */
export function overlayShown(
  layout: LayoutStoreState,
  ui: Pick<UiState, "managerOpen" | "officeOpen" | "hubOpen">,
  bots: Pick<BotSettings, "enabled">,
  hub: Pick<HubState, "enabled">,
): boolean {
  return managerShown(layout, ui) || officeShown(ui, bots) || hubShown(ui, hub);
}
