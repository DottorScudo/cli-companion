import { useEffect } from "react";
import { refreshIntegrations } from "../app/actions/integrations";
import { report } from "../lib/errors";
import { useTauriEvent } from "./useTauriEvent";

/**
 * Integrazioni nello store `configStore`: lette all'avvio e a ogni
 * `integrations-changed` (abilitazione o predefinita cambiate, anche altrove).
 */
export function useIntegrationsSync(): void {
  useEffect(() => {
    refreshIntegrations().catch((e) => report("caricamento delle integrazioni", e, { notify: true }));
  }, []);

  useTauriEvent("integrations-changed", () => {
    refreshIntegrations().catch((e) => report("aggiornamento delle integrazioni", e));
  });
}
