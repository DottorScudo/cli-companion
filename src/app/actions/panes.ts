// Azioni sui pannelli della griglia: attivazione, massimizza, fissa e
// trascinamento delle intestazioni (riordino nel gruppo).

import { reorderTabs, setActivePane, toggleMaximize, togglePin } from "../../lib/layout";
import { layoutStore, paneLayoutsOf } from "../../stores/layout";
import { paneDragStore } from "../../stores/paneDrag";

export function activatePane(key: string): void {
  layoutStore.apply((l) => setActivePane(l, key));
}

export function toggleMaximizePane(key: string): void {
  layoutStore.apply((l) => toggleMaximize(l, key));
}

export function togglePinPane(key: string): void {
  layoutStore.apply((l) => togglePin(l, key));
}

export function paneDragStart(id: string): void {
  paneDragStore.start(id);
}

/** Il puntatore è sopra `id`: bersaglio solo se è un altro pannello dello stesso gruppo. */
export function paneDragOver(id: string): void {
  paneDragStore.dragOver(id, paneLayoutsOf(layoutStore.getState().layout));
}

export function paneDragLeave(id: string): void {
  paneDragStore.leave(id);
}

export function paneDragEnd(): void {
  paneDragStore.end();
}

export function paneDrop(id: string): void {
  const { from } = paneDragStore.getState();
  if (from && from !== id) layoutStore.apply((l) => reorderTabs(l, from, id));
  paneDragStore.end();
}
