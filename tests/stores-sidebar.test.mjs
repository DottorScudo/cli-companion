import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  INITIAL_SIDEBAR,
  askArchive,
  askRemove,
  beginBusy,
  cancelArchive,
  cancelRemove,
  createSidebarStore,
  dragOver,
  endBusy,
  endDrag,
  expand,
  rowView,
  setEditingName,
  setError,
  startDrag,
  startEdit,
  stopEdit,
} from "../src/components/sidebar/sidebarState.ts";

describe("sidebarState: rinomina e conferme", () => {
  it("avviare la rinomina azzera errore e conferme di ogni riga", () => {
    let s = askRemove(INITIAL_SIDEBAR, "a");
    s = setError(s, "boom");
    s = startEdit(s, "b", "Beta");
    assert.equal(s.editingId, "b");
    assert.equal(s.editingName, "Beta");
    assert.equal(s.error, null);
    assert.equal(s.confirmingId, null);
    assert.equal(s.archivingId, null);
  });

  it("rimozione e archiviazione si escludono a vicenda e azzerano l'errore", () => {
    let s = setError(askRemove(INITIAL_SIDEBAR, "a"), "boom");
    s = askArchive(s, "b");
    assert.equal(s.confirmingId, null);
    assert.equal(s.archivingId, "b");
    assert.equal(s.error, null);
    s = askRemove(s, "c");
    assert.equal(s.archivingId, null);
    assert.equal(s.confirmingId, "c");
  });

  it("annullare tocca solo la propria conferma", () => {
    const s = askArchive(askRemove(INITIAL_SIDEBAR, "a"), "b");
    assert.equal(cancelRemove(s).archivingId, "b");
    assert.equal(cancelArchive(s).archivingId, null);
    assert.equal(stopEdit(startEdit(s, "a", "x")).editingId, null);
  });

  it("il nome in modifica si aggiorna senza toccare il resto", () => {
    const s = startEdit(INITIAL_SIDEBAR, "a", "Alfa");
    const t = setEditingName(s, "Alfa 2");
    assert.equal(t.editingName, "Alfa 2");
    assert.equal(t.editingId, "a");
  });
});

describe("sidebarState: operazione in corso", () => {
  it("beginBusy azzera l'errore e segna la riga; endBusy la libera", () => {
    const s = beginBusy(setError(INITIAL_SIDEBAR, "boom"), "a");
    assert.equal(s.busyId, "a");
    assert.equal(s.error, null);
    assert.equal(endBusy(s).busyId, null);
  });
});

describe("sidebarState: trascinamento", () => {
  it("la posizione di rilascio cambia solo se è diversa", () => {
    const s = dragOver(startDrag(INITIAL_SIDEBAR, "a"), "b", "before");
    assert.deepEqual(s.dropTarget, { id: "b", pos: "before" });
    assert.equal(dragOver(s, "b", "before"), s);
    assert.deepEqual(dragOver(s, "b", "after").dropTarget, { id: "b", pos: "after" });
  });

  it("endDrag azzera riga trascinata e posizione", () => {
    const s = endDrag(dragOver(startDrag(INITIAL_SIDEBAR, "a"), "b", "after"));
    assert.equal(s.dragId, null);
    assert.equal(s.dropTarget, null);
  });
});

describe("sidebarState: transizioni senza effetto", () => {
  it("ritornano lo stesso stato", () => {
    assert.equal(stopEdit(INITIAL_SIDEBAR), INITIAL_SIDEBAR);
    assert.equal(cancelRemove(INITIAL_SIDEBAR), INITIAL_SIDEBAR);
    assert.equal(cancelArchive(INITIAL_SIDEBAR), INITIAL_SIDEBAR);
    assert.equal(endBusy(INITIAL_SIDEBAR), INITIAL_SIDEBAR);
    assert.equal(endDrag(INITIAL_SIDEBAR), INITIAL_SIDEBAR);
    assert.equal(setError(INITIAL_SIDEBAR, null), INITIAL_SIDEBAR);
    const e = expand(INITIAL_SIDEBAR, "a");
    assert.equal(expand(e, "a"), e);
  });
});

describe("sidebarState: rowView", () => {
  it("espande una sola riga alla volta", () => {
    const s = expand(expand(INITIAL_SIDEBAR, "a"), "b");
    assert.equal(rowView(s, "a").expanded, false);
    assert.equal(rowView(s, "b").expanded, true);
  });

  it("i flag valgono solo per la riga interessata", () => {
    const s = beginBusy(askRemove(INITIAL_SIDEBAR, "a"), "a");
    assert.deepEqual(rowView(s, "a"), {
      editing: false,
      confirming: true,
      archiving: false,
      busy: true,
      expanded: false,
      dragging: false,
      drop: null,
    });
    assert.deepEqual(rowView(s, "b"), rowView(INITIAL_SIDEBAR, "b"));
  });

  it("la barra di rilascio compare sulla riga bersaglio, non su quella trascinata", () => {
    const s = dragOver(startDrag(INITIAL_SIDEBAR, "a"), "b", "after");
    assert.equal(rowView(s, "b").drop, "after");
    assert.equal(rowView(s, "a").drop, null);
    assert.equal(rowView(s, "a").dragging, true);
    assert.equal(rowView(s, "c").drop, null);
    // Senza trascinamento in corso non si disegna nulla, anche con un bersaglio residuo.
    assert.equal(rowView({ ...s, dragId: null }, "b").drop, null);
  });
});

describe("sidebarState: archivio", () => {
  it("le azioni aggiornano lo stato e notificano solo se cambia", () => {
    const store = createSidebarStore();
    let calls = 0;
    store.subscribe(() => calls++);
    store.expand("a");
    store.expand("a");
    assert.equal(calls, 1);
    store.startEdit("a", "Alfa");
    assert.equal(store.getState().editingName, "Alfa");
    store.endDrag();
    assert.equal(calls, 2);
  });
});
