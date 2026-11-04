// Stato dell'interfaccia: dialoghi, Tasks Manager, Ufficio, avviso della
// sidebar e flag di avvio. Ogni transizione ritorna lo stesso stato se non
// cambia nulla.

import { createStore } from "../lib/store.ts";
import type { OutdatedPlugin } from "../lib/tauri.ts";
import type { SettingsTab } from "../components/SettingsDialog.tsx";
import type { TasksManagerIntent } from "../components/tasks/TasksManager.tsx";

export interface SettingsState {
  open: boolean;
  tab: SettingsTab;
  /** Workspace selezionato nelle pagine per-workspace. */
  workspaceId?: string;
}

export interface UiState {
  addWorkspaceOpen: boolean;
  settings: SettingsState;
  /** Modalità per workspace: true = si mostra il Tasks Manager al posto dei terminali. */
  managerOpen: Readonly<Record<string, boolean>>;
  /** Richiesta di apertura dal pannello Task (dettaglio di un task / "Nuovo task"). */
  managerIntent: TasksManagerIntent | null;
  /** Ultimo `nonce` assegnato a una richiesta. */
  intentNonce: number;
  /** L'Ufficio è globale (non per workspace). */
  officeOpen: boolean;
  /** Avviso temporaneo (es. preset Task Intake non disponibile), mostrato nella sidebar. */
  notice: string | null;
  updateAvailable: boolean;
  outdatedPlugins: OutdatedPlugin[];
  /** Sessione ripristinata (o assente): l'app è pronta. */
  sessionReady: boolean;
}

export const DEFAULT_SETTINGS_TAB: SettingsTab = "general-launches";

export const INITIAL_UI: UiState = {
  addWorkspaceOpen: false,
  settings: { open: false, tab: DEFAULT_SETTINGS_TAB },
  managerOpen: {},
  managerIntent: null,
  intentNonce: 0,
  officeOpen: false,
  notice: null,
  updateAvailable: false,
  outdatedPlugins: [],
  sessionReady: false,
};

/** Un dialogo è aperto: le scorciatoie globali non fanno nulla. */
export function modalOpen(s: UiState): boolean {
  return s.settings.open || s.addWorkspaceOpen;
}

export function setAddWorkspaceOpen(s: UiState, open: boolean): UiState {
  return s.addWorkspaceOpen === open ? s : { ...s, addWorkspaceOpen: open };
}

/** Apre le Impostazioni su una pagina (e su un workspace, per quelle per-workspace). */
export function openSettings(s: UiState, tab: SettingsTab, workspaceId?: string): UiState {
  const cur = s.settings;
  if (cur.open && cur.tab === tab && cur.workspaceId === workspaceId) return s;
  return { ...s, settings: { open: true, tab, workspaceId } };
}

/** Chiude le Impostazioni; pagina e workspace restano, finché la riapertura non li imposta. */
export function closeSettings(s: UiState): UiState {
  return s.settings.open ? { ...s, settings: { ...s.settings, open: false } } : s;
}

/**
 * Apre il Tasks Manager di un workspace, con un'eventuale richiesta (nuovo
 * `nonce`: serve a riarmare l'effetto del manager). Chiude l'Ufficio, che
 * altrimenti lo coprirebbe. Senza richiesta azzera quella pendente.
 */
export function openManager(s: UiState, id: string, intent?: Omit<TasksManagerIntent, "nonce">): UiState {
  if (s.managerOpen[id] === true && !s.officeOpen && !intent && s.managerIntent === null) return s;
  const nonce = intent ? s.intentNonce + 1 : s.intentNonce;
  return {
    ...s,
    officeOpen: false,
    managerOpen: { ...s.managerOpen, [id]: true },
    managerIntent: intent ? { ...intent, nonce } : null,
    intentNonce: nonce,
  };
}

/** Chiude il Tasks Manager di un workspace (torna ai suoi terminali). */
export function closeManager(s: UiState, id: string): UiState {
  return s.managerOpen[id] ? { ...s, managerOpen: { ...s.managerOpen, [id]: false } } : s;
}

/** La richiesta di apertura vale una volta: il manager l'ha letta, poi si azzera. */
export function consumeIntent(s: UiState): UiState {
  return s.managerIntent === null ? s : { ...s, managerIntent: null };
}

export function setOffice(s: UiState, open: boolean): UiState {
  return s.officeOpen === open ? s : { ...s, officeOpen: open };
}

export function toggleOffice(s: UiState): UiState {
  return { ...s, officeOpen: !s.officeOpen };
}

export function setNotice(s: UiState, text: string | null): UiState {
  return s.notice === text ? s : { ...s, notice: text };
}

export function setUpdateAvailable(s: UiState, available: boolean): UiState {
  return s.updateAvailable === available ? s : { ...s, updateAvailable: available };
}

export function setOutdatedPlugins(s: UiState, list: OutdatedPlugin[]): UiState {
  return s.outdatedPlugins === list ? s : { ...s, outdatedPlugins: list };
}

export function setSessionReady(s: UiState, ready: boolean): UiState {
  return s.sessionReady === ready ? s : { ...s, sessionReady: ready };
}

export function createUiStore() {
  const store = createStore<UiState>(INITIAL_UI);
  return {
    ...store,
    setAddWorkspaceOpen: (open: boolean) => store.update((s) => setAddWorkspaceOpen(s, open)),
    openSettings: (tab: SettingsTab, workspaceId?: string) => store.update((s) => openSettings(s, tab, workspaceId)),
    closeSettings: () => store.update(closeSettings),
    openManager: (id: string, intent?: Omit<TasksManagerIntent, "nonce">) =>
      store.update((s) => openManager(s, id, intent)),
    closeManager: (id: string) => store.update((s) => closeManager(s, id)),
    consumeIntent: () => store.update(consumeIntent),
    setOffice: (open: boolean) => store.update((s) => setOffice(s, open)),
    toggleOffice: () => store.update(toggleOffice),
    setNotice: (text: string | null) => store.update((s) => setNotice(s, text)),
    setUpdateAvailable: (available: boolean) => store.update((s) => setUpdateAvailable(s, available)),
    setOutdatedPlugins: (list: OutdatedPlugin[]) => store.update((s) => setOutdatedPlugins(s, list)),
    setSessionReady: (ready: boolean) => store.update((s) => setSessionReady(s, ready)),
  };
}

export const uiStore = createUiStore();
