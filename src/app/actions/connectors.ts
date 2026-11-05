// Connettori dell'Agents HUB: caricamento di descrittori, istanze, regole e stato
// di salute, e le operazioni dell'utente. Lo stato sta in `stores/connectors.ts`;
// qui l'I/O col backend. Funziona anche con l'HUB spento (le istanze si
// configurano comunque). Le operazioni rifiutano con l'errore del backend: lo
// mostra chi le chiama, accanto a ciò che l'utente stava facendo.

import { api } from "../../lib/tauri";
import type {
  ConnectorActionId,
  ConnectorActionOutcome,
  ConnectorRoute,
  ConnectorRouteInput,
  ConnectorStatus,
} from "../../lib/tauri";
import { report } from "../../lib/errors";
import { noticeStore } from "../../lib/notices";
import { connectorsStore } from "../../stores/connectors";

/** Descrittori, istanze, regole e stato di salute. Non rifiuta mai: l'errore è già segnalato. */
export async function loadConnectors(): Promise<void> {
  try {
    const [descriptors, instances, routes] = await Promise.all([
      api.connectorListDescriptors(),
      api.connectorListInstances(),
      api.connectorListRoutes(null, null),
    ]);
    connectorsStore.descriptorsSet(descriptors);
    connectorsStore.instancesSet(instances);
    connectorsStore.routesSet(routes);
  } catch (e) {
    report("caricamento dei connettori", e);
  }
}

/** `connector-status`: l'elenco completo degli stati di salute, a ogni cambiamento. */
export function onConnectorStatus(list: ConnectorStatus[]): void {
  connectorsStore.statusesSet(list);
}

// ---------------------------------------------------------------- istanze

/** Crea un'istanza (disabilitata) con etichetta e configurazione senza segreti. */
export async function createInstance(connector: string, label: string, config: Record<string, unknown>): Promise<void> {
  connectorsStore.instanceUpserted(await api.connectorCreateInstance(connector, label, config));
}

export async function updateInstance(id: string, label: string, config: Record<string, unknown>): Promise<void> {
  connectorsStore.instanceUpserted(await api.connectorUpdateInstance(id, label, config));
}

export async function setInstanceEnabled(id: string, enabled: boolean): Promise<void> {
  connectorsStore.instanceUpserted(await api.connectorSetEnabled(id, enabled));
}

/** Elimina l'istanza (e le sue regole). Ciò che non si è potuto pulire (un segreto nel portachiavi) è un avviso che resta. */
export async function deleteInstance(id: string): Promise<void> {
  const outcome = await api.connectorDeleteInstance(id);
  connectorsStore.instanceRemoved(id);
  if (outcome.warning !== null) {
    noticeStore.push({ key: `connector-delete:${id}`, level: "warning", text: outcome.warning, sticky: true });
  }
}

/** Imposta o sostituisce un segreto: il valore passa solo da qui e non torna mai. */
export async function setSecret(id: string, slot: string, value: string): Promise<void> {
  connectorsStore.instanceUpserted(await api.connectorSetSecret(id, slot, value));
}

export async function clearSecret(id: string, slot: string): Promise<void> {
  connectorsStore.instanceUpserted(await api.connectorClearSecret(id, slot));
}

export function testConnection(id: string): Promise<ConnectorActionOutcome> {
  return api.connectorTest(id);
}

export function runConnectorAction(id: string, action: ConnectorActionId): Promise<ConnectorActionOutcome> {
  return api.connectorRunAction(id, action);
}

// ---------------------------------------------------------------- regole

export async function createRoute(instance: string, input: ConnectorRouteInput): Promise<void> {
  connectorsStore.routeUpserted(await api.connectorCreateRoute(instance, input));
}

export async function updateRoute(id: string, input: ConnectorRouteInput): Promise<void> {
  connectorsStore.routeUpserted(await api.connectorUpdateRoute(id, input));
}

export async function deleteRoute(route: ConnectorRoute): Promise<void> {
  await api.connectorDeleteRoute(route.id);
  connectorsStore.routeRemoved(route.id);
}

/** Rilegge le regole (una si disabilita da sola se il suo thread sparisce). Non rifiuta mai. */
export async function reloadRoutes(): Promise<void> {
  try {
    connectorsStore.routesSet(await api.connectorListRoutes(null, null));
  } catch (e) {
    report("caricamento delle regole dei connettori", e);
  }
}
