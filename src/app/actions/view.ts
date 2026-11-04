// Cosa si vede: workspace selezionato, Tasks Manager, Ufficio e avviso della
// sidebar. Funzioni di modulo (stabili): leggono lo stato dagli archivi con
// `getState()`, nessuna copia in ref.

import {
  activePaneId,
  focusTab,
  selectGlobal as selectGlobalView,
  selectGroup,
  toggleMaximize,
} from "../../lib/layout";
import { configStore } from "../../stores/config";
import { hasGroup, layoutStore, viewWorkspaceId } from "../../stores/layout";
import { tabsStore } from "../../stores/tabs";
import { uiStore } from "../../stores/ui";
import type { TasksManagerIntent } from "../../components/tasks/TasksManager";
import { officeShown } from "../../stores/view";
import { loadBoard } from "../boards";

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

/** Passa alla vista Globale (se ha tab fissati). */
export function selectGlobal(): void {
  layoutStore.apply(selectGlobalView);
}

/**
 * Apre il Tasks Manager di un workspace (lo seleziona se non è quello
 * mostrato), con un'eventuale richiesta (dettaglio di un task, "Nuovo task").
 * Solo stato: i terminali restano montati. Chiude l'Ufficio, che lo coprirebbe.
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
 * Con l'Ufficio aperto apre il manager (e chiude l'Ufficio): non lo chiude
 * "sotto" l'Ufficio dove non si vedrebbe.
 */
export function toggleManager(id: string): void {
  const shown = officeShown(uiStore.getState(), configStore.getState().botSettings);
  if (!shown && viewWorkspaceId(layoutStore.getState()) === id && uiStore.getState().managerOpen[id] === true) {
    uiStore.closeManager(id);
    return;
  }
  openManager(id);
}

/** Ufficio: Ctrl+Maiusc+O e pulsante della sidebar. */
export function toggleOffice(): void {
  uiStore.toggleOffice();
}

export function closeOffice(): void {
  uiStore.setOffice(false);
}

/** Ctrl+Maiusc+M: massimizza/ripristina il pannello attivo della vista mostrata. */
export function toggleMaximizeActivePane(): void {
  layoutStore.apply((l) => {
    const id = activePaneId(l);
    return id ? toggleMaximize(l, id) : l;
  });
}
