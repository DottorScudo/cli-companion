import { describe, it, before, after } from "node:test";
import assert from "node:assert/strict";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  assertValidVersion,
  bumpCargoLock,
  bumpCargoToml,
  bumpPackageJson,
  bumpPackageLock,
  bumpVersion,
} from "../scripts/bump-version.mjs";

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const VERSIONED_FILES = ["package.json", "package-lock.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock"];

describe("bump-version: validazione", () => {
  it("accetta x.y.z e prerelease", () => {
    assert.doesNotThrow(() => assertValidVersion("0.18.0"));
    assert.doesNotThrow(() => assertValidVersion("1.0.0-beta.1"));
  });

  for (const bad of [undefined, "", "1.2", "v1.2.3", "1.2.3.4", "1.2.3; rm -rf /", "1.2.3-"]) {
    it(`rifiuta ${JSON.stringify(bad)}`, () => {
      assert.throws(() => assertValidVersion(bad), /Versione non valida/);
    });
  }
});

describe("bump-version: trasformazioni dei singoli file", () => {
  it("package.json: cambia solo version e rispetta CRLF", () => {
    const raw = '{\r\n  "name": "x",\r\n  "version": "1.0.0"\r\n}\r\n';
    assert.equal(bumpPackageJson(raw, "1.1.0"), '{\r\n  "name": "x",\r\n  "version": "1.1.0"\r\n}\r\n');
  });

  it("package-lock.json: aggiorna testata e voce radice, non le dipendenze", () => {
    const raw = JSON.stringify(
      {
        name: "x",
        version: "1.0.0",
        packages: { "": { name: "x", version: "1.0.0" }, "node_modules/a": { version: "1.0.0" } },
      },
      null,
      2,
    );
    const lock = JSON.parse(bumpPackageLock(raw, "2.0.0"));
    assert.equal(lock.version, "2.0.0");
    assert.equal(lock.packages[""].version, "2.0.0");
    assert.equal(lock.packages["node_modules/a"].version, "1.0.0");
  });

  it("package-lock.json: errore se manca la voce radice", () => {
    assert.throws(() => bumpPackageLock('{"version":"1.0.0","packages":{}}', "2.0.0"), /voce radice/);
  });

  it("Cargo.toml: cambia la version del package e non quelle delle dipendenze", () => {
    const raw = '[package]\nname = "x"\nversion = "1.0.0"\n\n[dependencies]\ntauri = { version = "2" }\n';
    assert.equal(bumpCargoToml(raw, "1.2.3"), raw.replace('version = "1.0.0"', 'version = "1.2.3"'));
  });

  it("Cargo.lock: cambia solo la voce cli-companion", () => {
    const raw =
      '[[package]]\nname = "other"\nversion = "1.0.0"\n\n[[package]]\nname = "cli-companion"\nversion = "1.0.0"\n';
    assert.equal(
      bumpCargoLock(raw, "1.2.3"),
      '[[package]]\nname = "other"\nversion = "1.0.0"\n\n[[package]]\nname = "cli-companion"\nversion = "1.2.3"\n',
    );
  });

  it("Cargo.lock: errore se la voce cli-companion manca", () => {
    assert.throws(() => bumpCargoLock('[[package]]\nname = "other"\nversion = "1.0.0"\n', "1.2.3"), /trovate 0/);
  });
});

describe("bump-version: sul repository", () => {
  let sandbox;

  before(() => {
    sandbox = mkdtempSync(path.join(os.tmpdir(), "bump-version-"));
    for (const file of VERSIONED_FILES) {
      mkdirSync(path.dirname(path.join(sandbox, file)), { recursive: true });
      cpSync(path.join(root, file), path.join(sandbox, file));
    }
  });

  after(() => rmSync(sandbox, { recursive: true, force: true }));

  const readSandbox = (file) => readFileSync(path.join(sandbox, file), "utf8");

  it("la versione corrente non produce modifiche", () => {
    const current = JSON.parse(readSandbox("package.json")).version;
    assert.deepEqual(bumpVersion(current, sandbox), []);
  });

  it("una versione nuova aggiorna tutti i file e ripetuta non cambia nulla", () => {
    assert.deepEqual(bumpVersion("9.8.7", sandbox).sort(), [...VERSIONED_FILES].sort());
    assert.equal(JSON.parse(readSandbox("package.json")).version, "9.8.7");
    const lock = JSON.parse(readSandbox("package-lock.json"));
    assert.equal(lock.version, "9.8.7");
    assert.equal(lock.packages[""].version, "9.8.7");
    assert.match(readSandbox("src-tauri/Cargo.toml"), /^version = "9\.8\.7"$/m);
    assert.match(readSandbox("src-tauri/Cargo.lock"), /name = "cli-companion"\r?\nversion = "9\.8\.7"/);
    assert.deepEqual(bumpVersion("9.8.7", sandbox), []);
  });

  it("una versione non valida non tocca i file", () => {
    const before = readSandbox("package.json");
    assert.throws(() => bumpVersion("nope", sandbox), /Versione non valida/);
    assert.equal(readSandbox("package.json"), before);
  });
});
