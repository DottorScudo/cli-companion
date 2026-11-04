import { configStore } from "../stores/config";
import { hubStore } from "../stores/hub";
import { layoutStore, viewWorkspaceId } from "../stores/layout";
import { uiStore } from "../stores/ui";
import { hubShown, managerShown, officeShown, overlayShown, sidebarShown } from "../stores/view";
import { useStore, useStores } from "./useStore";

// Cosa è mostrato, derivato dagli archivi (le funzioni pure stanno in
// `stores/view.ts`, usate anche dalle azioni). Gli archivi sono costanti di
// modulo: `useStores` li vuole stabili.

const LAYOUT_UI = [layoutStore, uiStore] as const;
const UI_CONFIG = [uiStore, configStore] as const;
const UI_HUB = [uiStore, hubStore] as const;
const VIEW_STORES = [layoutStore, uiStore, configStore, hubStore] as const;

const computeManagerShown = () => managerShown(layoutStore.getState(), uiStore.getState());
const computeOfficeShown = () => officeShown(uiStore.getState(), configStore.getState().botSettings);
const computeHubShown = () => hubShown(uiStore.getState(), hubStore.getState());
const computeSidebarShown = () => sidebarShown(uiStore.getState(), hubStore.getState());
const computeOverlayShown = () =>
  overlayShown(layoutStore.getState(), uiStore.getState(), configStore.getState().botSettings, hubStore.getState());

/** Workspace mostrato (quello a fuoco senza gruppo, altrimenti il gruppo attivo; null con la vista globale). */
export function useViewWorkspaceId(): string | null {
  return useStore(layoutStore, viewWorkspaceId);
}

/** Il Tasks Manager del workspace mostrato copre pannello Task e terminali. */
export function useManagerShown(): boolean {
  return useStores(LAYOUT_UI, computeManagerShown);
}

/** L'Ufficio è aperto e i bot sono attivi. */
export function useOfficeShown(): boolean {
  return useStores(UI_CONFIG, computeOfficeShown);
}

/** L'Agents HUB è aperto e acceso. */
export function useHubShown(): boolean {
  return useStores(UI_HUB, computeHubShown);
}

/** La sidebar dei workspace è visibile (nascosta mentre l'Agents HUB è aperto). */
export function useSidebarShown(): boolean {
  return useStores(UI_HUB, computeSidebarShown);
}

/** Manager, Ufficio o Agents HUB coprono pannello Task e terminali. */
export function useOverlayShown(): boolean {
  return useStores(VIEW_STORES, computeOverlayShown);
}
