import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  activeWorld,
  hubShown,
  managerShown,
  officeShown,
  overlayShown,
  sidebarFooterShown,
  worldRailShown,
} from "../src/stores/view.ts";
import { INITIAL_LAYOUT_STATE } from "../src/stores/layout.ts";
import { INITIAL_UI } from "../src/stores/ui.ts";

const BOTS_ON = { enabled: true };
const BOTS_OFF = { enabled: false };
const HUB_ON = { enabled: true };
const HUB_OFF = { enabled: false };

const layoutOf = (activeWorkspaceId, activeView = "workspace", focusedWorkspaceId = null) => ({
  layout: { ...INITIAL_LAYOUT_STATE.layout, activeWorkspaceId, activeView },
  focusedWorkspaceId,
});

describe("view: officeShown", () => {
  it("serve l'Ufficio aperto e i bot attivi", () => {
    assert.equal(officeShown({ officeOpen: true }, BOTS_ON), true);
    assert.equal(officeShown({ officeOpen: true }, BOTS_OFF), false);
    assert.equal(officeShown({ officeOpen: false }, BOTS_ON), false);
  });
});

describe("view: managerShown", () => {
  const ui = { ...INITIAL_UI, managerOpen: { a: true, b: false } };

  it("segue il workspace mostrato", () => {
    assert.equal(managerShown(layoutOf("a"), ui), true);
    assert.equal(managerShown(layoutOf("b"), ui), false);
    assert.equal(managerShown(layoutOf("c"), ui), false);
  });

  it("il workspace a fuoco senza gruppo ha la precedenza", () => {
    assert.equal(managerShown(layoutOf("b", "workspace", "a"), ui), true);
  });

  it("mai nella vista globale", () => {
    assert.equal(managerShown(layoutOf("a", "global"), ui), false);
  });

  it("nessun workspace: niente manager", () => {
    assert.equal(managerShown(layoutOf(null), ui), false);
  });
});

describe("view: activeWorld", () => {
  it("l'Agents HUB solo scelto e acceso, altrimenti i terminali", () => {
    assert.equal(activeWorld({ world: "hub" }, HUB_ON), "hub");
    assert.equal(activeWorld({ world: "hub" }, HUB_OFF), "terminals");
    assert.equal(activeWorld({ world: "terminals" }, HUB_ON), "terminals");
    assert.equal(activeWorld({ world: "terminals" }, HUB_OFF), "terminals");
  });
});

describe("view: hubShown", () => {
  it("serve il mondo HUB scelto e l'HUB acceso", () => {
    assert.equal(hubShown({ world: "hub" }, HUB_ON), true);
    assert.equal(hubShown({ world: "hub" }, HUB_OFF), false);
    assert.equal(hubShown({ world: "terminals" }, HUB_ON), false);
  });
});

describe("view: barra dei mondi e piede della sidebar", () => {
  it("la barra c'è solo con l'HUB acceso", () => {
    assert.equal(worldRailShown(HUB_ON), true);
    assert.equal(worldRailShown(HUB_OFF), false);
  });

  it("il piede della sidebar c'è solo senza la barra", () => {
    assert.equal(sidebarFooterShown(HUB_ON), false);
    assert.equal(sidebarFooterShown(HUB_OFF), true);
  });
});

describe("view: overlayShown", () => {
  it("manager, Ufficio o Agents HUB", () => {
    const ui = { ...INITIAL_UI, managerOpen: { a: true }, officeOpen: false };
    assert.equal(overlayShown(layoutOf("a"), ui, BOTS_ON, HUB_OFF), true);
    assert.equal(overlayShown(layoutOf("b"), ui, BOTS_ON, HUB_OFF), false);
    assert.equal(overlayShown(layoutOf("b"), { ...ui, officeOpen: true }, BOTS_ON, HUB_OFF), true);
    assert.equal(overlayShown(layoutOf("b"), { ...ui, officeOpen: true }, BOTS_OFF, HUB_OFF), false);
    assert.equal(overlayShown(layoutOf("b"), { ...ui, world: "hub" }, BOTS_OFF, HUB_ON), true);
    assert.equal(overlayShown(layoutOf("b"), { ...ui, world: "hub" }, BOTS_OFF, HUB_OFF), false);
  });
});
