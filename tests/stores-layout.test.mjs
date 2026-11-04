import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  EMPTY_LAYOUT,
  GLOBAL_ID,
  computePaneLayouts,
  openTab,
  pinTab,
  selectGlobal,
  selectGroup,
  toggleMaximize,
} from "../src/lib/layout.ts";
import {
  INITIAL_LAYOUT_STATE,
  applyLayout,
  createLayoutStore,
  focusWorkspace,
  hasGroup,
  paneLayoutsOf,
  samePaneLayout,
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

  it("il focus senza gruppo decade solo se cambia il gruppo selezionato", () => {
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
