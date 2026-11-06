// Cosa si vede: mondo (Terminali o Agents HUB), workspace selezionato, Tasks
// Manager, Ufficio e avviso della sidebar. Funzioni di modulo (stabili):
// leggono lo stato dagli archivi con `getState()`, nessuna copia in ref.

import {
  activePaneId,
  focusTab,
  selectGlobal as selectGlobalView,
  selectGroup,
  toggleMaximize,
} from "../../lib/layout";
import { configStore } from "../../stores/config";
import { hasGroup, layoutStore, shownLayout, viewWorkspaceId } from "../../stores/layout";
import { tabsStore } from "../../stores/tabs";
import { uiStore } from "../../stores/ui";
import type { TasksManagerIntent } from "../../components/tasks/TasksManager";
import { hubShown, officeShown } from "../../stores/view";
import { hubStore } from "../../stores/hub";
import { hubChannelIdsOf } from "../../stores/hubChannels";
import { workspacesStore } from "../../stores/workspaces";
import { latestWaitingChannel } from "../../lib/hub";
import { loadBoard } from "../boards";
import { selectChannel } from "./hub";

/** Durata dell'avviso temporaneo nella sidebar. */
const NOTICE_MS = 6000;

let noticeTimer: ReturnType<typeof setTimeout> | undefined;

/** Avviso temporaneo nella sidebar. */
export function showNotice(text: string): void {
  uiStore.setNotice(text);
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => uiStore.setNotice(null), NOTICE_MS);
}

/**
 * Un terminale messo a fuoco riporta ai terminali del suo workspace: il
 * Tasks Manager di quel workspace si chiude e il focus "senza gruppo" decade.
 */
export function showTerminalsOf(owner: string | undefined): void {
  if (owner) uiStore.closeManager(owner);
  layoutStore.focusWorkspace(null);
}

/**
 * Un tab messo a fuoco da pannello Task, Tasks Manager o notifica: torna ai
 * terminali del suo workspace (la chiave del tab è l'id di sessione del worker).
 */
export function focusTerminal(key: string): void {
  // Tab chiuso nel frattempo: niente da mettere a fuoco, la vista resta com'è.
  const tab = tabsStore.get(key);
  if (!tab) return;
  uiStore.setOffice(false);
  uiStore.setWorld("terminals");
  showTerminalsOf(tab.workspaceId);
  layoutStore.apply((l) => focusTab(l, key));
}

/**
 * Seleziona un workspace. Uno senza terminali non ha un gruppo: lo si
 * "mostra" comunque (con la sua modalità, utile per aprirne il Tasks Manager).
 */
export function selectWorkspace(id: string): void {
  layoutStore.focusWorkspace(hasGroup(layoutStore.getState().layout, id) ? null : id);
  layoutStore.apply((l) => selectGroup(l, id));
}

/**
 * Passa alla vista Globale (se ha tab fissati). Il focus "senza gruppo" decade
 * anche se il layout era già sulla vista globale, nascosta dal focus.
 */
export function selectGlobal(): void {
  if (layoutStore.getState().layout.global.order.length === 0) return;
  layoutStore.focusWorkspace(null);
  layoutStore.apply(selectGlobalView);
}

/**
 * Apre il Tasks Manager di un workspace (lo seleziona se non è quello
 * mostrato), con un'eventuale richiesta (dettaglio di un task, "Nuovo task").
 * Solo stato: i terminali restano montati. Porta ai terminali e chiude l'Ufficio, che lo coprirebbe.
 */
export function openManager(id: string, intent?: Omit<TasksManagerIntent, "nonce">): void {
  const { layout } = layoutStore.getState();
  if (viewWorkspaceId(layoutStore.getState()) !== id) {
    if (hasGroup(layout, id)) {
      layoutStore.focusWorkspace(null);
      layoutStore.apply((l) => selectGroup(l, id));
    } else {
      layoutStore.focusWorkspace(id);
    }
  }
  loadBoard(id);
  uiStore.openManager(id, intent);
}

/**
 * Sidebar e Ctrl+Maiusc+B: apre il manager, o lo chiude se è già aperto.
 * Con Ufficio o Agents HUB aperti apre il manager (e torna ai terminali): non
 * lo chiude "sotto" una vista che lo copre, dove non si vedrebbe.
 */
export function toggleManager(id: string): void {
  const ui = uiStore.getState();
  const covered = officeShown(ui, configStore.getState().botSettings) || hubShown(ui, hubStore.getState());
  if (!covered && viewWorkspaceId(layoutStore.getState()) === id && uiStore.getState().managerOpen[id] === true) {
    uiStore.closeManager(id);
    return;
  }
  openManager(id);
}

/** Ufficio: Ctrl+Maiusc+O e pulsante. Aprirlo dall'Agents HUB riporta ai terminali (lo fa lo store). */
export function toggleOffice(): void {
  uiStore.toggleOffice();
}

export function closeOffice(): void {
  uiStore.setOffice(false);
}

/** Ctrl+Maiusc+H: passa dai terminali all'Agents HUB e viceversa (entrando chiude l'Ufficio). */
export function toggleWorld(): void {
  uiStore.toggleWorld();
}

/** Barra dei mondi: torna ai terminali (chiude anche l'Ufficio, che li coprirebbe). */
export function showTerminals(): void {
  uiStore.setOffice(false);
  uiStore.setWorld("terminals");
}

/** Barra dei mondi: mondo dell'Agents HUB. */
export function showHub(): void {
  uiStore.setWorld("hub");
}

/** Apre l'Agents HUB sul canale dato. */
export function openHubChannel(channel: string): void {
  uiStore.setWorld("hub");
  selectChannel(channel);
}

/** Apre l'Agents HUB sul canale del thread in attesa più recente, altrimenti sull'ultimo canale aperto. */
export function openHubWaiting(): void {
  const { threads, channel } = hubStore.getState();
  const ids = hubChannelIdsOf(workspacesStore.getState());
  openHubChannel(latestWaitingChannel(threads, ids) ?? channel);
}

/** Dall'Agents HUB ai terminali di un workspace: chiude il suo Tasks Manager e lo seleziona. */
export function showWorkspaceTerminals(id: string): void {
  uiStore.setWorld("terminals");
  uiStore.closeManager(id);
  selectWorkspace(id);
}

/**
 * Ctrl+Maiusc+M: massimizza/ripristina il pannello attivo della vista
 * mostrata. Workspace mostrato senza terminali: nessun pannello.
 */
export function toggleMaximizeActivePane(): void {
  const id = activePaneId(shownLayout(layoutStore.getState()));
  if (id) layoutStore.apply((l) => toggleMaximize(l, id));
}
