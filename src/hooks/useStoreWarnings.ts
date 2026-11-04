import { useEffect } from "react";
import { api } from "../lib/tauri";
import { report } from "../lib/errors";
import { noticeStore } from "../lib/notices";
import { storeWarningNotice } from "../lib/storeWarnings";
import type { StoreWarning } from "../lib/storeWarnings";
import { useTauriEvent } from "./useTauriEvent";

const showWarning = (warning: StoreWarning) => noticeStore.push(storeWarningNotice(warning));

/**
 * Mostra gli avvisi di persistenza del backend: quelli già accumulati
 * all'avvio (`store_warnings`) e i nuovi (`store-warning`). La chiave
 * dell'avviso evita i doppioni quando i due percorsi si sovrappongono.
 */
export function useStoreWarnings() {
  useTauriEvent("store-warning", showWarning);
  useEffect(() => {
    let cancelled = false;
    api
      .storeWarnings()
      .then((list) => {
        if (!cancelled) list.forEach(showWarning);
      })
      .catch((err) => report("lettura degli avvisi di salvataggio", err));
    return () => {
      cancelled = true;
    };
  }, []);
}
