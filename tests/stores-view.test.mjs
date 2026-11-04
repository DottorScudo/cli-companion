import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { hubShown, managerShown, officeShown, overlayShown } from "../src/stores/view.ts";
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

describe("view: hubShown", () => {
  it("serve l'HUB aperto e acceso", () => {
    assert.equal(hubShown({ hubOpen: true }, HUB_ON), true);
    assert.equal(hubShown({ hubOpen: true }, HUB_OFF), false);
    assert.equal(hubShown({ hubOpen: false }, HUB_ON), false);
  });
});

describe("view: overlayShown", () => {
  it("manager, Ufficio o Agents HUB", () => {
    const ui = { ...INITIAL_UI, managerOpen: { a: true }, officeOpen: false };
    assert.equal(overlayShown(layoutOf("a"), ui, BOTS_ON, HUB_OFF), true);
    assert.equal(overlayShown(layoutOf("b"), ui, BOTS_ON, HUB_OFF), false);
    assert.equal(overlayShown(layoutOf("b"), { ...ui, officeOpen: true }, BOTS_ON, HUB_OFF), true);
    assert.equal(overlayShown(layoutOf("b"), { ...ui, officeOpen: true }, BOTS_OFF, HUB_OFF), false);
    assert.equal(overlayShown(layoutOf("b"), { ...ui, hubOpen: true }, BOTS_OFF, HUB_ON), true);
    assert.equal(overlayShown(layoutOf("b"), { ...ui, hubOpen: true }, BOTS_OFF, HUB_OFF), false);
  });
});
