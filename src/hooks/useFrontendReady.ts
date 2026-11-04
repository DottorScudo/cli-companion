import { useEffect, useRef } from "react";
import { api } from "../lib/tauri";
import { report } from "../lib/errors";
import { uiStore } from "../stores/ui";
import { useStore } from "./useStore";

/**
 * Dice al backend che il frontend è pronto: sessione ripristinata e
 * ascoltatori degli eventi registrati. Solo da quel momento parte il timer
 * dell'automazione (i worker aprono i loro tab, che il ripristino unisce a
 * quelli presenti). Una volta sola.
 */
export function useFrontendReady(eventsReady: boolean): void {
  const sessionReady = useStore(uiStore, (s) => s.sessionReady);
  const sentRef = useRef(false);
  useEffect(() => {
    if (!sessionReady || !eventsReady || sentRef.current) return;
    sentRef.current = true;
    api.frontendReady().catch((e) => report("segnalazione di app pronta al backend", e));
  }, [sessionReady, eventsReady]);
}
