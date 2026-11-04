import { useEffect } from "react";
import { modalOpen, uiStore } from "../../stores/ui";

/**
 * Ctrl+Maiusc+O apre/chiude l'Ufficio. In fase di cattura, come Ctrl+Maiusc+B,
 * cosi' xterm non vede il tasto. Con un dialogo aperto non fa nulla (lo legge
 * da `uiStore`); con `enabled` falso (bot spenti) non registra niente.
 * `toggle` deve essere stabile (una funzione di modulo): il listener si
 * registra una volta per cambio di `enabled`.
 */
export function useOfficeShortcut(enabled: boolean, toggle: () => void): void {
  useEffect(() => {
    if (!enabled) return;
    const onKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey || !e.shiftKey || e.altKey || e.metaKey) return;
      if (e.key.toLowerCase() !== "o") return;
      if (modalOpen(uiStore.getState())) return;
      e.preventDefault();
      e.stopPropagation();
      toggle();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [enabled, toggle]);
}
