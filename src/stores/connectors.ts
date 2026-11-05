// Connettori dell'Agents HUB (docs/CONNECTORS.MD): descrittori dei connettori
// compilati nell'app, istanze configurate (con gli slot dei segreti impostati,
// mai i valori) e stato di salute di ciascuna. Ogni transizione ritorna lo stesso
// stato se non cambia nulla. Funziona anche con l'HUB spento (le istanze si
// configurano comunque; si avviano solo con l'HUB acceso).

import { createStore, shallowEqual } from "../lib/store.ts";
import type { ConnectorDescriptor } from "../bindings/ConnectorDescriptor.ts";
import type { ConnectorInstanceView } from "../bindings/ConnectorInstanceView.ts";
import type { ConnectorStatus } from "../bindings/ConnectorStatus.ts";

export interface ConnectorsState {
  /** I connettori compilati nell'app; vuoto finché non arrivano (o se non ce n'è nessuno). */
  descriptors: readonly ConnectorDescriptor[];
  /** Le istanze configurate, nell'ordine del file. */
  instances: readonly ConnectorInstanceView[];
  /** Lo stato di salute per id istanza: l'ultimo elenco ricevuto (comando o evento `connector-status`). */
  statuses: Readonly<Record<string, ConnectorStatus>>;
  /** Le istanze e i descrittori sono stati letti almeno una volta. */
  loaded: boolean;
}

export const INITIAL_CONNECTORS: ConnectorsState = { descriptors: [], instances: [], statuses: {}, loaded: false };

const byInstance = (list: readonly ConnectorStatus[]): Record<string, ConnectorStatus> =>
  Object.fromEntries(list.map((s) => [s.instance, s]));

const sameStatuses = (
  a: Readonly<Record<string, ConnectorStatus>>,
  b: Readonly<Record<string, ConnectorStatus>>,
): boolean => {
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every((k) => k in b && shallowEqual(a[k], b[k]));
};

export function setDescriptors(s: ConnectorsState, descriptors: readonly ConnectorDescriptor[]): ConnectorsState {
  return { ...s, descriptors };
}

/** L'elenco intero delle istanze (con il loro stato): sostituisce tutto e segna lo stato come letto. */
export function setInstances(s: ConnectorsState, instances: readonly ConnectorInstanceView[]): ConnectorsState {
  return { ...s, instances, statuses: byInstance(instances.map((i) => i.status)), loaded: true };
}

/** Un'istanza creata o cambiata: sostituisce quella con lo stesso id, altrimenti si aggiunge in coda. */
export function upsertInstance(s: ConnectorsState, view: ConnectorInstanceView): ConnectorsState {
  const id = view.instance.id;
  const exists = s.instances.some((i) => i.instance.id === id);
  const instances = exists ? s.instances.map((i) => (i.instance.id === id ? view : i)) : [...s.instances, view];
  return { ...s, instances, statuses: { ...s.statuses, [id]: view.status } };
}

/** Un'istanza eliminata: via lei e il suo stato. */
export function removeInstance(s: ConnectorsState, id: string): ConnectorsState {
  if (!s.instances.some((i) => i.instance.id === id) && !(id in s.statuses)) return s;
  const { [id]: _removed, ...statuses } = s.statuses;
  return { ...s, instances: s.instances.filter((i) => i.instance.id !== id), statuses };
}

/** L'elenco completo degli stati di salute (il comando `connector_status` o l'evento): sostituisce quello di prima. */
export function setStatuses(s: ConnectorsState, list: readonly ConnectorStatus[]): ConnectorsState {
  const statuses = byInstance(list);
  return sameStatuses(s.statuses, statuses) ? s : { ...s, statuses };
}

/** Lo stato di un'istanza: quello ricevuto, altrimenti quello letto con l'istanza. */
export function statusOf(s: ConnectorsState, id: string): ConnectorStatus | null {
  return s.statuses[id] ?? s.instances.find((i) => i.instance.id === id)?.status ?? null;
}

export function createConnectorsStore() {
  const store = createStore<ConnectorsState>(INITIAL_CONNECTORS);
  return {
    ...store,
    descriptorsSet: (descriptors: readonly ConnectorDescriptor[]) =>
      store.update((s) => setDescriptors(s, descriptors)),
    instancesSet: (instances: readonly ConnectorInstanceView[]) => store.update((s) => setInstances(s, instances)),
    instanceUpserted: (view: ConnectorInstanceView) => store.update((s) => upsertInstance(s, view)),
    instanceRemoved: (id: string) => store.update((s) => removeInstance(s, id)),
    statusesSet: (list: readonly ConnectorStatus[]) => store.update((s) => setStatuses(s, list)),
  };
}

export const connectorsStore = createConnectorsStore();
