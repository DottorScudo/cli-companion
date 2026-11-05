// Connettori dell'Agents HUB: caricamento di descrittori, istanze e stato di
// salute. Lo stato sta in `stores/connectors.ts`; qui l'I/O col backend. Funziona
// anche con l'HUB spento (le istanze si configurano comunque).

import { api } from "../../lib/tauri";
import type { ConnectorStatus } from "../../lib/tauri";
import { report } from "../../lib/errors";
import { connectorsStore } from "../../stores/connectors";

/** Descrittori, istanze e stato di salute. Non rifiuta mai: l'errore è già segnalato. */
export async function loadConnectors(): Promise<void> {
  try {
    const [descriptors, instances] = await Promise.all([api.connectorListDescriptors(), api.connectorListInstances()]);
    connectorsStore.descriptorsSet(descriptors);
    connectorsStore.instancesSet(instances);
  } catch (e) {
    report("caricamento dei connettori", e);
  }
}

/** `connector-status`: l'elenco completo degli stati di salute, a ogni cambiamento. */
export function onConnectorStatus(list: ConnectorStatus[]): void {
  connectorsStore.statusesSet(list);
}
