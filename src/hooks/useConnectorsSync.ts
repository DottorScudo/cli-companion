import { useEffect } from "react";
import { loadConnectors, onConnectorStatus } from "../app/actions/connectors";
import { useTauriEvent } from "./useTauriEvent";

/** Connettori: l'elenco all'avvio, poi `connector-status` tiene aggiornato lo stato di salute. */
export function useConnectorsSync(): void {
  useTauriEvent("connector-status", onConnectorStatus);
  useEffect(() => {
    void loadConnectors();
  }, []);
}
