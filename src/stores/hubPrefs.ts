// Preferenze dell'Agents HUB salvate in localStorage: la modalità di vista, il
// pannello del thread e i messaggi visti per thread. Si scrive solo quando il
// valore cambia.

import { writeSeen } from "../lib/hubUnread.ts";
import { writeThreadPanelShown } from "../lib/hubThreadPanelPref.ts";
import { writeViewMode } from "../lib/hubViewMode.ts";
import type { Store } from "../lib/store.ts";
import type { StorageLike } from "../lib/taskPanelPrefs.ts";
import type { HubState } from "./hub.ts";

/** Salva le preferenze a ogni cambiamento dello stato; storage assente = solo in memoria. */
export function persistHubPrefs(store: Store<HubState>, storage: StorageLike | null): void {
  if (storage === null) return;
  let { viewMode, threadPanelShown, lastSeen } = store.getState();
  store.subscribe(() => {
    const next = store.getState();
    if (next.viewMode !== viewMode) writeViewMode(storage, next.viewMode);
    if (next.threadPanelShown !== threadPanelShown) writeThreadPanelShown(storage, next.threadPanelShown);
    if (next.lastSeen !== lastSeen) writeSeen(storage, next.lastSeen);
    ({ viewMode, threadPanelShown, lastSeen } = next);
  });
}
