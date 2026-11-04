// Cosa è mostrato: il mondo attivo (Terminali o Agents HUB), la barra dei
// mondi, il Tasks Manager del workspace in vista e l'Ufficio al posto dei
// terminali. Funzioni pure sullo stato degli archivi, condivise da hook
// (`hooks/useView.ts`) e azioni (`app/actions/*`).

import type { BotSettings } from "../lib/tauri.ts";
import { viewWorkspaceId } from "./layout.ts";
import type { LayoutStoreState } from "./layout.ts";
import type { HubState } from "./hub.ts";
import type { UiState, World } from "./ui.ts";

/** L'Ufficio è globale (non per workspace) e vive solo con i bot attivi. */
export function officeShown(ui: Pick<UiState, "officeOpen">, bots: Pick<BotSettings, "enabled">): boolean {
  return ui.officeOpen && bots.enabled;
}

/** Il mondo mostrato: l'Agents HUB solo con l'interruttore acceso, altrimenti sempre i terminali. */
export function activeWorld(ui: Pick<UiState, "world">, hub: Pick<HubState, "enabled">): World {
  return ui.world === "hub" && hub.enabled ? "hub" : "terminals";
}

/** Si è nel mondo dell'Agents HUB. */
export function hubShown(ui: Pick<UiState, "world">, hub: Pick<HubState, "enabled">): boolean {
  return activeWorld(ui, hub) === "hub";
}

/** La barra dei mondi esiste solo con l'Agents HUB acceso. */
export function worldRailShown(hub: Pick<HubState, "enabled">): boolean {
  return hub.enabled;
}

/** Senza barra dei mondi, Impostazioni, Ufficio e versione stanno nel piede della sidebar. */
export function sidebarFooterShown(hub: Pick<HubState, "enabled">): boolean {
  return !worldRailShown(hub);
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
  ui: Pick<UiState, "managerOpen" | "officeOpen" | "world">,
  bots: Pick<BotSettings, "enabled">,
  hub: Pick<HubState, "enabled">,
): boolean {
  return managerShown(layout, ui) || officeShown(ui, bots) || hubShown(ui, hub);
}
