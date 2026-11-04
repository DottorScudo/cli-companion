import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { storeWarningNotice } from "../src/lib/storeWarnings.ts";

const warning = (extra = {}) => ({
  file: "session.json",
  kind: "corrupt",
  message: "Il file session.json non è utilizzabile.",
  backup: "session.json.bak-1",
  at: 1700000000000,
  ...extra,
});

describe("storeWarningNotice", () => {
  it("è un avviso sticky con il testo del backend", () => {
    const notice = storeWarningNotice(warning());
    assert.equal(notice.level, "warning");
    assert.equal(notice.sticky, true);
    assert.equal(notice.text, "Il file session.json non è utilizzabile.");
  });

  it("la chiave distingue file e istante, così lo stesso avviso non si duplica", () => {
    assert.equal(storeWarningNotice(warning()).key, storeWarningNotice(warning()).key);
    assert.notEqual(storeWarningNotice(warning()).key, storeWarningNotice(warning({ at: 1 })).key);
    assert.notEqual(storeWarningNotice(warning()).key, storeWarningNotice(warning({ file: "other.json" })).key);
  });
});
