// Gestori degli eventi del backend (`agent-event`, `notification-activated`,
// `automation-*`). Registrati una volta sola da `hooks/useAppEvents.ts`: lo
// stato lo leggono dagli archivi con `getState()`, non da copie.

import { api } from "../lib/tauri";
import type { AgentEvent, AutomationClose, AutomationSpawn, AutomationTabInfo } from "../lib/tauri";
import { report } from "../lib/errors";
import { notificationBody, shouldSystemNotify, soundForEvent } from "../lib/agents";
import { hubCoversAgentEvent } from "../lib/hubNotify";
import { openTabBackground } from "../lib/layout";
import { playAttention, playDing } from "../lib/notifySound";
import { notifyIfUnfocused } from "../lib/systemNotify";
import { configStore } from "../stores/config";
import { hubStore, reservedBotNames } from "../stores/hub";
import { layoutStore } from "../stores/layout";
import { tabLabel } from "../stores/selectors";
import { runtimeStore } from "../stores/tabRuntime";
import { tabsStore } from "../stores/tabs";
import { workspacesStore } from "../stores/workspaces";
import { closeTab } from "./actions/tabs";
import { focusTerminal } from "./actions/view";
import { onAgentEventUsage } from "./usage";

/**
 * Eventi dei coding agent: stato del tab, suono e notifica di sistema.
 * Con le notifiche dell'HUB attive restano solo lo stato (✓/❗) e l'uso: suono
 * e toast sono quelli dell'HUB. `test` è il self-test delle Impostazioni: solo
 * il suono, sempre. Gli eventi il cui tab non esiste (più) sono ignorati.
 */
export function onAgentEvent(ev: AgentEvent): void {
  if (ev.event === "test") {
    playDing();
    return;
  }
  const tab = ev.tab_id ? tabsStore.get(ev.tab_id) : undefined;
  if (!tab) return;
  runtimeStore.agentEvent(tab.key, ev);
  onAgentEventUsage(tab.key, ev.event, ev.transcript_path, ev.agent);
  // Con le notifiche dell'HUB attive, suono e toast sono solo i suoi.
  if (hubCoversAgentEvent(hubStore.getState())) return;
  const sound = soundForEvent(ev.event);
  if (sound === "attention") playAttention();
  else if (sound === "ding") playDing();
  // I worker dei task notificano solo via `task-notify` (attenzione, fallito).
  if (shouldSystemNotify(ev.event) && !tab.automated) {
    const ws = workspacesStore.get(tab.workspaceId);
    const label = tabLabel(tab, runtimeStore.get(tab.key), ws, configStore.getState().builtinLaunches);
    void notifyIfUnfocused(tab.key, ws?.name ?? "", notificationBody(ev.event, label, ev.message));
  }
}

/**
 * Click su una notifica di sistema: la finestra è già in primo piano
 * (backend); qui si seleziona workspace e pannello. Solo stato del layout:
 * i TerminalView non si smontano né cambiano genitore. Tab chiuso: niente.
 */
export function onNotificationActivated(e: { tab_id: string }): void {
  focusTerminal(e.tab_id);
}

/** Il backend chiede di aprire il tab di un worker. */
export function onAutomationSpawn(ev: AutomationSpawn): void {
  // Workspace sconosciuto (rimosso nel frattempo) o tab già aperto: ignora.
  const ws = workspacesStore.get(ev.workspace_id);
  if (!ws || tabsStore.get(ev.tab_id)) return;
  runtimeStore.seed(ev.tab_id, {
    automation: { title: ev.title, status: "starting", agentId: ev.agent_id, cwd: ev.cwd },
  });
  // Apertura sincrona: un secondo evento identico non riapre il tab.
  tabsStore.open(
    {
      key: ev.tab_id,
      workspaceId: ws.id,
      command: "",
      startCommand: ev.command,
      automated: true,
      cwd: ev.cwd,
    },
    configStore.getState().botSettings.names,
    reservedBotNames(hubStore.getState()),
  );
  // In secondo piano: non toglie all'utente la vista in cui sta lavorando.
  layoutStore.apply((l) => openTabBackground(l, ws.id, ev.tab_id));
}

export function onAutomationClose(ev: AutomationClose): void {
  closeTab(ev.tab_id);
}

export function onAutomationUpdate(ev: AutomationTabInfo): void {
  runtimeStore.automationUpdate(ev.tab_id, ev);
}

/** Riallinea i metadati dei tab automatici già aperti (es. dopo un ricaricamento). */
export async function realignAutomationSessions(): Promise<void> {
  try {
    for (const info of await api.automationSessions()) runtimeStore.automationUpdate(info.tab_id, info);
  } catch (e) {
    report("allineamento dei tab automatici", e);
  }
}
