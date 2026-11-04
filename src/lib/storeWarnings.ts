import type { StoreWarning } from "../bindings/StoreWarning.ts";
import type { NoticeInput } from "./notices.ts";

export type { StoreWarning };

/** Avviso per l'utente: il testo è già in italiano dal backend; resta finché non lo chiude. */
export function storeWarningNotice(warning: StoreWarning): NoticeInput {
  return {
    key: `store:${warning.file}:${warning.at}`,
    level: "warning",
    text: warning.message,
    sticky: true,
  };
}
