import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  ATTACHMENTS_MAX,
  ATTACHMENT_SIZE_MAX,
  admitFiles,
  baseName,
  canSend,
  draftFileName,
  draftLabel,
  draftStaged,
  formatSize,
  hasPreview,
  isUploading,
  pasteAttaches,
  readyAttachmentIds,
} from "../src/lib/hubAttachments.ts";

const attachment = (id, extra = {}) => ({ id, name: `${id}.png`, mime: "image/png", size: 10, ...extra });
const draft = (key, staged = null, extra = {}) => ({
  key,
  name: `${key}.txt`,
  size: null,
  preview: null,
  attachment: staged,
  ...extra,
});
const candidate = (name, size) => ({ name, size });

describe("hubAttachments: limiti come il backend", () => {
  it("5 allegati da 10 MB (hub/attachments.rs)", () => {
    assert.equal(ATTACHMENTS_MAX, 5);
    assert.equal(ATTACHMENT_SIZE_MAX, 10 * 1024 * 1024);
  });
});

describe("hubAttachments: anteprima", () => {
  it("solo i formati d'immagine che un <img> mostra, senza badare alle maiuscole", () => {
    for (const mime of ["image/png", "image/jpeg", "image/gif", "image/webp", "image/bmp", "IMAGE/PNG"]) {
      assert.equal(hasPreview(mime), true, mime);
    }
    for (const mime of ["image/svg+xml", "application/pdf", "text/plain", "application/octet-stream", ""]) {
      assert.equal(hasPreview(mime), false, mime);
    }
  });
});

describe("hubAttachments: dimensioni", () => {
  it("byte interi, poi una cifra decimale con la virgola sotto le dieci unità", () => {
    assert.equal(formatSize(0), "0 B");
    assert.equal(formatSize(512), "512 B");
    assert.equal(formatSize(1023), "1023 B");
    assert.equal(formatSize(1024), "1 KB");
    assert.equal(formatSize(1536), "1,5 KB");
    assert.equal(formatSize(20 * 1024), "20 KB");
    assert.equal(formatSize(ATTACHMENT_SIZE_MAX), "10 MB");
    assert.equal(formatSize(3.25 * 1024 * 1024), "3,3 MB");
    assert.equal(formatSize(5 * 1024 ** 4), "5120 GB");
  });

  it("valori negativi come zero", () => {
    assert.equal(formatSize(-5), "0 B");
  });
});

describe("hubAttachments: nomi", () => {
  it("baseName: ultimo componente, Windows o POSIX", () => {
    assert.equal(baseName("C:\\Users\\me\\Desktop\\nota.pdf"), "nota.pdf");
    assert.equal(baseName("/home/me/foto.png"), "foto.png");
    assert.equal(baseName("cartella\\"), "cartella");
    assert.equal(baseName("solo.txt"), "solo.txt");
    assert.equal(baseName(""), "");
  });

  it("draftFileName: il nome del file, o uno di ripiego con l'estensione del tipo", () => {
    assert.equal(draftFileName("  screenshot.png ", "image/png"), "screenshot.png");
    assert.equal(draftFileName("", "image/png"), "allegato.png");
    assert.equal(draftFileName(" ", "image/jpeg"), "allegato.jpg");
    assert.equal(draftFileName("", "application/x-sconosciuto"), "allegato");
    assert.equal(draftFileName("", ""), "allegato");
  });
});

describe("hubAttachments: incolla", () => {
  it("allega solo file senza testo negli appunti", () => {
    assert.equal(pasteAttaches(["Files"], 1), true);
    assert.equal(pasteAttaches(["Files", "text/html"], 2), true);
    assert.equal(pasteAttaches(["text/plain", "Files"], 1), false);
    assert.equal(pasteAttaches(["text/plain"], 0), false);
    assert.equal(pasteAttaches([], 0), false);
  });
});

describe("hubAttachments: ammissione", () => {
  it("entrano tutti se c'è posto", () => {
    const list = [candidate("a", 1), candidate("b", null)];
    assert.deepEqual(admitFiles(0, list), { accepted: list, error: null });
  });

  it("10 MB esatti sì, un byte in più no, con il nome nel messaggio", () => {
    const ok = candidate("ok.bin", ATTACHMENT_SIZE_MAX);
    const big = candidate("grande.iso", ATTACHMENT_SIZE_MAX + 1);
    const result = admitFiles(0, [ok, big]);
    assert.deepEqual(result.accepted, [ok]);
    assert.match(result.error, /«grande\.iso» supera i 10 MB/);
  });

  it("la dimensione ignota (dialog) la controlla il backend", () => {
    assert.equal(admitFiles(0, [candidate("x", null)]).accepted.length, 1);
  });

  it("oltre il quinto allegato: entrano i primi, gli altri restano fuori con un messaggio", () => {
    const list = ["a", "b", "c"].map((n) => candidate(n, 1));
    const result = admitFiles(ATTACHMENTS_MAX - 2, list);
    assert.deepEqual(result.accepted, list.slice(0, 2));
    assert.match(result.error, /al massimo 5 allegati/);
    assert.deepEqual(admitFiles(ATTACHMENTS_MAX, list).accepted, []);
    assert.deepEqual(admitFiles(ATTACHMENTS_MAX + 3, list).accepted, []);
  });

  it("entrambi gli errori insieme; i file troppo grandi non occupano posti", () => {
    const big = candidate("grande", ATTACHMENT_SIZE_MAX + 1);
    const small = ["a", "b"].map((n) => candidate(n, 1));
    const result = admitFiles(ATTACHMENTS_MAX - 1, [big, ...small]);
    assert.deepEqual(result.accepted, [small[0]]);
    assert.match(result.error, /supera.*al massimo/);
  });

  it("nessun file, nessun errore", () => {
    assert.deepEqual(admitFiles(0, []), { accepted: [], error: null });
  });
});

describe("hubAttachments: bozze del compositore", () => {
  it("in caricamento finché non ci sono i metadati dello staging", () => {
    assert.equal(isUploading(draft("d1")), true);
    assert.equal(isUploading(draft("d1", attachment("att-1-1"))), false);
  });

  it("readyAttachmentIds: gli id pronti nell'ordine dei chip", () => {
    const list = [draft("d1", attachment("att-1-2")), draft("d2"), draft("d3", attachment("att-1-1"))];
    assert.deepEqual(readyAttachmentIds(list), ["att-1-2", "att-1-1"]);
    assert.deepEqual(readyAttachmentIds([]), []);
  });

  it("draftStaged: nome e dimensione definitivi; una bozza tolta nel frattempo resta tolta", () => {
    const staged = attachment("att-1-1", { name: "pulito.png", size: 42 });
    const list = [draft("d1"), draft("d2")];
    const next = draftStaged(list, "d2", staged);
    assert.deepEqual(next[1], { ...list[1], name: "pulito.png", size: 42, attachment: staged });
    assert.equal(next[0], list[0]);
    assert.deepEqual(draftStaged(list, "d9", staged), list);
  });

  it("canSend: testo o allegati, mai durante un invio o un caricamento", () => {
    const ready = [draft("d1", attachment("att-1-1"))];
    assert.equal(canSend("ciao", [], false), true);
    assert.equal(canSend("  \n", [], false), false);
    assert.equal(canSend("", ready, false), true);
    assert.equal(canSend("ciao", ready, true), false);
    assert.equal(canSend("ciao", [...ready, draft("d2")], false), false);
  });

  it("draftLabel: nome, dimensione se nota, stato del caricamento", () => {
    assert.equal(draftLabel(draft("d1")), "d1.txt, in caricamento");
    assert.equal(draftLabel(draft("d1", null, { size: 1536 })), "d1.txt, 1,5 KB, in caricamento");
    assert.equal(draftLabel(draft("d1", attachment("att-1-1"), { size: 10 })), "d1.txt, 10 B");
  });
});
