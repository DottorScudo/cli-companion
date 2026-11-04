// Dialoghi e impostazioni: apertura di Impostazioni e "Aggiungi workspace",
// e i valori che le Impostazioni riportano all'app.

import type { BotSettings, Workspace } from "../../lib/tauri";
import { report } from "../../lib/errors";
import { refreshIntegrations } from "./integrations";
import { configStore } from "../../stores/config";
import { uiStore } from "../../stores/ui";

/** Impostazioni: sulla pagina "Aggiornamenti" se ce n'è uno disponibile (pallino acceso). */
export function openSettings(): void {
  uiStore.openSettings(uiStore.getState().updateAvailable ? "updates" : "general-launches");
}

/** Impostazioni sulla pagina Integrazioni. */
export function openIntegrationSettings(): void {
  uiStore.openSettings("integrations");
}

/** Impostazioni sugli avvii del workspace. */
export function openLaunchSettings(ws: Workspace): void {
  uiStore.openSettings("launches", ws.id);
}

export function closeSettings(): void {
  uiStore.closeSettings();
}

export function openAddWorkspace(): void {
  uiStore.setAddWorkspaceOpen(true);
}

export function closeAddWorkspace(): void {
  uiStore.setAddWorkspaceOpen(false);
}

export function setShowUsage(showUsage: boolean): void {
  configStore.patch({ showUsage });
}

export function setGlobalLaunch(globalLaunch: string | null): void {
  configStore.patch({ globalLaunch });
}

export function setBotSettings(botSettings: BotSettings): void {
  configStore.patch({ botSettings });
}

export function closePluginUpdate(): void {
  uiStore.setOutdatedPlugins([]);
  // Dopo un aggiornamento non seguito da riavvio lo stato va riletto.
  refreshIntegrations().catch((e) => report("rilevamento dei pacchetti installati", e));
}
