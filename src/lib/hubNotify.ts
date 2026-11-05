// Agents HUB (docs/HUB.MD › Notifiche): quando una notifica del backend
// (`hub-notify`) va mostrata e quando l'HUB si prende quelle dei task. Funzioni
// pure a zero dipendenze, importabili sia dall'app sia dai test `node --test`.

import type { HubNoticeSound } from "../bindings/HubNoticeSound.ts";

/** Cosa guarda l'utente adesso. */
export interface HubViewport {
  /** La finestra dell'app e' in primo piano. */
  focused: boolean;
  /** Si e' nel mondo dell'Agents HUB (non nei Terminali). */
  hubShown: boolean;
  channel: string;
  threadId: string | null;
}

/** Il thread a cui si riferisce una notifica. */
export interface HubThreadTarget {
  channel: string;
  thread_id: string;
}

/**
 * Il thread e' aperto e visibile: finestra in primo piano, mondo dell'HUB e
 * proprio quel canale e quel thread. Il messaggio e' davanti all'utente, quindi
 * ne' notifica ne' suono.
 */
export function threadIsInView(view: HubViewport, target: HubThreadTarget): boolean {
  return view.focused && view.hubShown && view.channel === target.channel && view.threadId === target.thread_id;
}

/**
 * Mostrare la notifica di sistema (e il suo suono): sempre, tranne se l'utente
 * sta gia' guardando quel thread. A finestra non in primo piano, nei Terminali
 * o su un altro thread si mostra.
 */
export function shouldShowHubNotice(view: HubViewport, target: HubThreadTarget): boolean {
  return !threadIsInView(view, target);
}

/** Il suono di una notifica: il ding (risposta, evento normale) o il richiamo (serve l'utente). */
export function soundOf(sound: HubNoticeSound): "ding" | "attention" {
  return sound === "attention" ? "attention" : "ding";
}

/** Tipi di `task-notify` che l'HUB riporta nei thread con la propria notifica (task fallito, in attenzione, domanda del worker). */
const HUB_COVERED_TASK_NOTICES: ReadonlySet<string> = new Set(["attention", "failed", "question"]);

/** Parte delle impostazioni che decide se l'HUB notifica. */
export interface HubNotifySettings {
  enabled: boolean;
  settings: { notifications: boolean } | null;
}

/** L'HUB e' acceso e le sue notifiche di sistema pure. */
export function hubNotifies(hub: HubNotifySettings): boolean {
  return hub.enabled && hub.settings?.notifications === true;
}

/**
 * `task-notify` va saltato: con le notifiche dell'HUB attive l'utente riceve
 * quell'evento una volta sola, dal thread (che apre al click). Spente le
 * notifiche, o spento l'HUB, `task-notify` torna a notificare da solo.
 */
export function hubCoversTaskNotice(hub: HubNotifySettings, kind: string): boolean {
  return hubNotifies(hub) && HUB_COVERED_TASK_NOTICES.has(kind);
}

/**
 * Gli eventi dei coding agent (`agent-event`) non danno ne' toast ne' suono:
 * con le notifiche dell'HUB attive le uniche notifiche sono quelle dell'HUB
 * (che include la fine di un agente di un worker). Resta lo stato del tab.
 */
export function hubCoversAgentEvent(hub: HubNotifySettings): boolean {
  return hubNotifies(hub);
}
