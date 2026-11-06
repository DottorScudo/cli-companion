import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  EMPTY_LAYOUT,
  GLOBAL_ID,
  activePaneId,
  closeTabIn,
  computePaneLayouts,
  focusTab,
  openTab,
  openTabBackground,
  pinTab,
  selectGlobal,
  selectGroup,
  setActivePane,
  toggleMaximize,
  unpinTab,
} from "../src/lib/layout.ts";
import {
  INITIAL_LAYOUT_STATE,
  applyLayout,
  createLayoutStore,
  focusWorkspace,
  hasGroup,
  paneLayoutsOf,
  samePaneLayout,
  shownLayout,
  shownPaneLayouts,
  viewWorkspaceId,
} from "../src/stores/layout.ts";

/** w1: a, b — w2: c (attivo). */
const layout = () => openTab(openTab(openTab(EMPTY_LAYOUT, "w1", "a"), "w1", "b"), "w2", "c");
const stateWith = (l, focusedWorkspaceId = null) => ({ layout: l, focusedWorkspaceId });

describe("layout store: applyLayout", () => {
  it("stesso layout = stesso stato", () => {
    const s = stateWith(layout(), "w9");
    assert.equal(
      applyLayout(s, (l) => l),
      s,
    );
    assert.equal(
      applyLayout(s, (l) => selectGroup(l, "w2")),
      s,
      "selezione già attiva",
    );
  });

  it("il focus senza gruppo decade se si sceglie un altro gruppo", () => {
    const s = stateWith(layout(), "w9");
    const next = applyLayout(s, (l) => selectGroup(l, "w1"));
    assert.equal(next.layout.activeWorkspaceId, "w1");
    assert.equal(next.focusedWorkspaceId, null);
  });

  it("il focus decade anche se cambia la vista (globale)", () => {
    const pinned = pinTab(layout(), "c");
    const s = stateWith(pinned, "w9");
    const next = applyLayout(s, selectGlobal);
    assert.equal(next.layout.activeView, "global");
    assert.equal(next.focusedWorkspaceId, null);
  });

  it("un cambio che non tocca la selezione lascia il focus", () => {
    const s = stateWith(layout(), "w9");
    const next = applyLayout(s, (l) => toggleMaximize(l, "c"));
    assert.notEqual(next.layout, s.layout);
    assert.equal(next.layout.groups[1].maximizedTabId, "c");
    assert.equal(next.focusedWorkspaceId, "w9");
  });

  it("il focus decade se si sceglie un pannello di un altro gruppo", () => {
    for (const fn of [(l) => setActivePane(l, "a"), (l) => focusTab(l, "a"), (l) => toggleMaximize(l, "a")]) {
      const next = applyLayout(stateWith(layout(), "w9"), fn);
      assert.equal(next.layout.activeWorkspaceId, "w1");
      assert.equal(next.focusedWorkspaceId, null);
    }
  });

  it("il gruppo attivo che sparisce (tab chiuso) non fa decadere il focus", () => {
    const next = applyLayout(stateWith(layout(), "w9"), (l) => closeTabIn(l, "c"));
    assert.equal(next.layout.activeWorkspaceId, "w1", "il layout passa al gruppo vicino");
    assert.equal(next.focusedWorkspaceId, "w9");
    assert.equal(viewWorkspaceId(next), "w9");
  });

  it("un tab in secondo piano non fa decadere il focus", () => {
    const fromEmpty = applyLayout(stateWith(EMPTY_LAYOUT, "w9"), (l) => openTabBackground(l, "w1", "a"));
    assert.equal(fromEmpty.layout.activeWorkspaceId, "w1", "primo gruppo: diventa l'attivo");
    assert.equal(fromEmpty.focusedWorkspaceId, "w9");
    const other = applyLayout(stateWith(layout(), "w9"), (l) => openTabBackground(l, "w3", "d"));
    assert.equal(other.focusedWorkspaceId, "w9");
  });

  it("la vista globale rimasta vuota non fa decadere il focus", () => {
    const s = stateWith(selectGlobal(pinTab(layout(), "c")), "w9");
    const next = applyLayout(s, (l) => unpinTab(l, "c"));
    assert.equal(next.layout.activeView, "workspace");
    assert.equal(next.focusedWorkspaceId, "w9");
  });
});

describe("layout store: layout mostrato", () => {
  it("senza focus (o con il focus sul gruppo attivo) è il layout stesso", () => {
    const l = layout();
    assert.equal(shownLayout(stateWith(l)), l);
    assert.equal(shownLayout(stateWith(l, "w2")), l);
    assert.equal(shownPaneLayouts(stateWith(l)), paneLayoutsOf(l));
  });

  it("focus senza gruppo: nessun pannello visibile o a fuoco, nessun pannello attivo", () => {
    const s = stateWith(layout(), "w9");
    const panes = shownPaneLayouts(s);
    assert.deepEqual(Object.keys(panes).sort(), ["a", "b", "c"], "i pannelli restano disposti (montati)");
    assert.ok(Object.values(panes).every((p) => !p.visible && !p.focused));
    assert.equal(shownLayout(s).activeWorkspaceId, "w9");
    assert.equal(activePaneId(shownLayout(s)), null);
  });

  it("focus senza gruppo vince anche sulla vista globale", () => {
    const s = stateWith(selectGlobal(pinTab(layout(), "c")), "w9");
    assert.equal(shownLayout(s).activeView, "workspace");
    assert.ok(Object.values(shownPaneLayouts(s)).every((p) => !p.visible));
  });

  it("gruppo nato in secondo piano nel workspace a fuoco: si vedono i suoi pannelli", () => {
    const s = applyLayout(stateWith(layout(), "w9"), (l) => openTabBackground(l, "w9", "d"));
    assert.equal(s.layout.activeWorkspaceId, "w2");
    const panes = shownPaneLayouts(s);
    assert.ok(panes.d.visible && panes.d.focused);
    assert.ok(!panes.a.visible && !panes.b.visible && !panes.c.visible);
    assert.equal(activePaneId(shownLayout(s)), "d");
  });

  it("è memoizzato per stato: stesso layout e focus, stessa istanza", () => {
    const l = layout();
    const shown = shownLayout(stateWith(l, "w9"));
    assert.equal(shownLayout(stateWith(l, "w9")), shown);
    assert.equal(shownPaneLayouts(stateWith(l, "w9")), shownPaneLayouts(stateWith(l, "w9")));
    assert.notEqual(shownLayout(stateWith(l, "w8")), shown);
    assert.equal(shownLayout(stateWith(l, "w8")).activeWorkspaceId, "w8");
  });

  it("tornando al gruppo di prima i pannelli ricompaiono come erano", () => {
    const l = toggleMaximize(layout(), "a");
    const before = paneLayoutsOf(l);
    const focused = stateWith(l, "w9");
    assert.ok(Object.values(shownPaneLayouts(focused)).every((p) => !p.visible));
    const back = applyLayout(focusWorkspace(focused, null), (x) => selectGroup(x, "w1"));
    assert.equal(back.layout, l, "il layout non è cambiato");
    assert.deepEqual(shownPaneLayouts(back), before);
  });
});

describe("layout store: focus e vista mostrata", () => {
  it("focusWorkspace: stesso id = stesso stato", () => {
    const s = INITIAL_LAYOUT_STATE;
    assert.equal(focusWorkspace(s, null), s);
    const focused = focusWorkspace(s, "w1");
    assert.equal(focused.focusedWorkspaceId, "w1");
    assert.equal(focusWorkspace(focused, "w1"), focused);
  });

  it("viewWorkspaceId: il focus vince, poi il gruppo attivo, null con la vista globale", () => {
    const l = pinTab(layout(), "c");
    assert.equal(viewWorkspaceId(stateWith(l)), "w2");
    assert.equal(viewWorkspaceId(stateWith(l, "w9")), "w9");
    assert.equal(viewWorkspaceId(stateWith(selectGlobal(l))), null);
    assert.equal(viewWorkspaceId(stateWith(selectGlobal(l), "w9")), "w9");
    assert.equal(viewWorkspaceId(INITIAL_LAYOUT_STATE), null);
  });

  it("hasGroup", () => {
    const l = layout();
    assert.ok(hasGroup(l, "w1"));
    assert.ok(!hasGroup(l, "w3"));
  });

  it("lo store notifica solo se qualcosa cambia", () => {
    const store = createLayoutStore();
    store.apply(() => layout());
    let calls = 0;
    store.subscribe(() => calls++);
    store.apply((l) => l);
    store.focusWorkspace(null);
    assert.equal(calls, 0);
    store.focusWorkspace("w9");
    store.apply((l) => selectGroup(l, "w1"));
    assert.equal(calls, 2);
    assert.equal(store.getState().focusedWorkspaceId, null);
  });
});

describe("paneLayoutsOf", () => {
  it("è memoizzato per oggetto layout e uguale a computePaneLayouts", () => {
    const l = layout();
    assert.equal(paneLayoutsOf(l), paneLayoutsOf(l));
    assert.deepEqual(paneLayoutsOf(l), computePaneLayouts(l));
    assert.notEqual(paneLayoutsOf({ ...l }), paneLayoutsOf(l));
  });
});

describe("samePaneLayout", () => {
  const base = () => paneLayoutsOf(layout()).c;

  it("stessa istanza o stessi valori", () => {
    const a = base();
    assert.ok(samePaneLayout(a, a));
    assert.ok(samePaneLayout(a, { ...a, rect: { ...a.rect } }));
    assert.ok(samePaneLayout(undefined, undefined));
  });

  it("assente da una parte sola", () => {
    assert.ok(!samePaneLayout(base(), undefined));
    assert.ok(!samePaneLayout(undefined, base()));
  });

  it("ogni campo conta", () => {
    const a = base();
    for (const change of [
      { visible: !a.visible },
      { focused: !a.focused },
      { maximized: !a.maximized },
      { pinned: !a.pinned },
      { groupWorkspaceId: GLOBAL_ID },
      { rect: null },
      { rect: { ...a.rect, left: a.rect.left + 1 } },
      { rect: { ...a.rect, top: a.rect.top + 1 } },
      { rect: { ...a.rect, width: a.rect.width - 1 } },
      { rect: { ...a.rect, height: a.rect.height - 1 } },
    ]) {
      assert.ok(!samePaneLayout(a, { ...a, ...change }), JSON.stringify(change));
    }
  });

  it("due rect null sono uguali", () => {
    const a = { ...base(), rect: null };
    assert.ok(samePaneLayout(a, { ...a }));
  });
});
