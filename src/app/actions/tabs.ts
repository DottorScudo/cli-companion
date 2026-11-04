// Apertura e chiusura dei terminali, e ciò che i TerminalView riportano
// (titolo, input, stato). Funzioni di modulo (stabili): leggono lo stato
// dagli archivi con `getState()`.

import { api } from "../../lib/tauri";
import type { BuiltinLaunch, Launch, Workspace } from "../../lib/tauri";
import { report } from "../../lib/errors";
import { INTAKE_LAUNCH_ID } from "../../lib/automation";
import { launchCommands, resolveDefaultLaunch } from "../../lib/launch";
import { closeTabIn, openTab } from "../../lib/layout";
import { configStore } from "../../stores/config";
import { layoutStore } from "../../stores/layout";
import { paneDragStore } from "../../stores/paneDrag";
import { runtimeStore } from "../../stores/tabRuntime";
import { tabsStore } from "../../stores/tabs";
import { workspacesStore } from "../../stores/workspaces";
import { isClosing } from "../lifecycle";
import { showNotice, showTerminalsOf } from "./view";

export function newTabKey(): string {
  try {
    return crypto.randomUUID();
  } catch {
    return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  }
}

/**
 * Senza `launch` parte l'avvio predefinito del workspace, se esiste;
 * altrimenti una shell semplice. Il nuovo terminale attiva il suo gruppo.
 */
export function spawnTab(ws: Workspace, launch?: Launch | BuiltinLaunch): void {
  const { builtinLaunches, globalLaunch, botSettings } = configStore.getState();
  const chosen = launch ?? resolveDefaultLaunch(ws, builtinLaunches, globalLaunch);
  const key = newTabKey();
  // Per gli integrati si salva il comando di ripristino, si esegue quello di avvio.
  const { command, startCommand } = launchCommands(chosen);
  showTerminalsOf(ws.id);
  runtimeStore.seed(key);
  // Apertura sincrona: due tab aperti di seguito non prendono lo stesso nome.
  tabsStore.open({ key, workspaceId: ws.id, launchId: chosen?.id, command, startCommand }, botSettings.names);
  layoutStore.apply((l) => openTab(l, ws.id, key));
}

/**
 * Task Intake: ogni richiesta apre un nuovo tab del preset nel workspace
 * (più chat in parallelo), con la stessa via delle voci rapide della sidebar.
 * Senza il preset tra gli avvii integrati (backend non aggiornato) avvisa.
 */
export function openIntake(workspaceId: string): void {
  const ws = workspacesStore.get(workspaceId);
  if (!ws) return;
  const launch = configStore.getState().builtinLaunches.find((b) => b.id === INTAKE_LAUNCH_ID);
  if (!launch) {
    showNotice("Task Intake non è disponibile: aggiorna il plugin dei coding agent");
    return;
  }
  spawnTab(ws, launch);
}

// Un tab automatico chiuso va comunicato al backend una volta sola, da
// qualunque via arrivi la chiusura (utente, `automation-close`, shell uscita).
const reportedClosed = new Set<string>();

/**
 * Chiusura di un terminale (dall'utente o perché la shell è uscita): la sua
 * cronologia salvata non serve più.
 */
export function closeTab(key: string): void {
  const automated = tabsStore.get(key)?.automated === true;
  if (automated && !reportedClosed.has(key)) {
    reportedClosed.add(key);
    void api.automationTabClosed(key).catch((e) => report("chiusura del tab automatico", e));
  }
  tabsStore.remove(key);
  layoutStore.apply((l) => closeTabIn(l, key));
  runtimeStore.remove(key);
  // Un trascinamento con il tab chiuso come origine o bersaglio non può più finire.
  const { from, over } = paneDragStore.getState();
  if (from === key || over === key) paneDragStore.end();
  // Gli automatici non salvano lo scrollback: niente da cancellare.
  if (!automated) void api.deleteScrollback(key).catch((e) => report("eliminazione della cronologia salvata", e));
}

/**
 * La shell è uscita da sola. Durante la chiusura dell'app i pty vengono
 * chiusi: non è un'uscita dell'utente, la sessione va conservata.
 */
export function handleExit(key: string): void {
  if (isClosing()) return;
  closeTab(key);
}

export function handleTitle(key: string, title: string): void {
  runtimeStore.setTitle(key, title);
}

/** Input dell'utente: Invio => l'agente lavora, altro input toglie ✓/❗. */
export function handleInput(key: string, enter: boolean): void {
  runtimeStore.userInput(key, enter);
}

export function handleStatus(key: string, busy: boolean): void {
  runtimeStore.setBusy(key, busy);
}
