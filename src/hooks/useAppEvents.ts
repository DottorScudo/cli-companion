import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { report } from "../lib/errors";
import { startSubscriptions } from "../lib/subscription";
import type { Unlisten } from "../lib/subscription";
import type { TauriEvents } from "../lib/tauri";
import {
  onAgentEvent,
  onAutomationClose,
  onAutomationSpawn,
  onAutomationUpdate,
  onNotificationActivated,
  realignAutomationSessions,
} from "../app/events";

const noop: Unlisten = () => undefined;

/**
 * Registrazione di un evento. Un errore si segnala con l'evento nel messaggio
 * e la sottoscrizione resta inattiva (così `ready` non resta mai in attesa).
 */
function subscribeTo<E extends keyof TauriEvents>(event: E, handler: (payload: TauriEvents[E]) => void) {
  return () =>
    listen<TauriEvents[E]>(event, (e) => handler(e.payload)).catch((err) => {
      report(`ascolto dell'evento ${event}`, err);
      return noop;
    });
}

/**
 * Ascolta gli eventi del backend gestiti da `app/events.ts` (agenti,
 * notifiche, sessioni automatiche) per tutta la vita dell'app e riallinea i
 * tab automatici già aperti. Restituisce `true` quando tutti gli ascoltatori
 * sono registrati: solo allora il backend può mandare eventi (`useFrontendReady`).
 */
export function useAppEvents(): boolean {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const subscriptions = startSubscriptions(
      [
        subscribeTo("agent-event", onAgentEvent),
        subscribeTo("notification-activated", onNotificationActivated),
        subscribeTo("automation-spawn", onAutomationSpawn),
        subscribeTo("automation-close", onAutomationClose),
        subscribeTo("automation-update", onAutomationUpdate),
      ],
      (err) => report("registrazione degli ascoltatori degli eventi", err),
    );
    void subscriptions.ready.then(() => {
      if (!cancelled) setReady(true);
    });
    return () => {
      cancelled = true;
      subscriptions.unlisten();
    };
  }, []);

  useEffect(() => {
    void realignAutomationSessions();
  }, []);

  return ready;
}
