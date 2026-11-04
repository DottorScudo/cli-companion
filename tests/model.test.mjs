import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { MODEL_MAX, modelError, sanitizeModel } from "../src/lib/model.ts";

describe("model: nome del modello", () => {
  it("trim, vuoto = null", () => {
    assert.equal(sanitizeModel("  opus "), "opus");
    assert.equal(sanitizeModel("   "), null);
    assert.equal(sanitizeModel("x".repeat(200)).length, MODEL_MAX);
  });
  it("accetta i nomi provider/modello e modello:tag", () => {
    for (const ok of [
      " opus ",
      "claude-opus-4.5_x",
      "opencode/big-pickle",
      "openrouter/anthropic/claude-x",
      "ollama/llama3:8b",
      "",
    ]) {
      assert.equal(modelError(ok), null, ok);
    }
  });
  it("rifiuta spazi, caratteri strani e forme non sicure, massimo 128", () => {
    for (const bad of ["a b", "a;b", "a$b", 'a"b', "a`b", "a|b", "a&b", "a%b", "a!b", "a\nb", "modèl"]) {
      assert.ok(modelError(bad), JSON.stringify(bad));
    }
    for (const bad of ["-x", "/x", "x/", "a//b"]) assert.ok(modelError(bad), bad);
    assert.ok(modelError("x".repeat(129)));
    assert.equal(modelError("x".repeat(128)), null);
    assert.equal(MODEL_MAX, 128);
  });
});
