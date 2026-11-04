import { report } from "../lib/errors";
import { createPtyKeeper } from "../lib/ptyKeeper";
import { api } from "../lib/tauri";
import type { PtyEvent } from "../lib/tauri";

/**
 * Le sessioni PTY dei tab, una per app. La chiusura di una sessione sganciata
 * aspetta il giro successivo dell'event loop: un rimontaggio sincrono (StrictMode,
 * Fast Refresh) la riprende prima.
 */
export const ptyKeeper = createPtyKeeper<PtyEvent>({
  close: (id) => api.ptyClose(id),
  defer: (fn) => {
    const timer = setTimeout(fn, 0);
    return () => clearTimeout(timer);
  },
  onError: report,
});
