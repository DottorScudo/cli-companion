import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { report } from "./errors";
import { startSubscription } from "./subscription";
import type { TauriEvents } from "./tauri";

/**
 * Ascolta un evento del backend per tutta la vita del componente. L'handler è
 * tenuto in un ref: può cambiare a ogni render senza riregistrare il listener,
 * e vede sempre lo stato più recente.
 */
export function useTauriEvent<E extends keyof TauriEvents>(event: E, handler: (payload: TauriEvents[E]) => void): void {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(
    () =>
      startSubscription(
        () => listen<TauriEvents[E]>(event, (e) => handlerRef.current(e.payload)),
        (err) => report(`ascolto dell'evento ${event}`, err),
      ),
    [event],
  );
}
