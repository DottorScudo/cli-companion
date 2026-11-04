import type { NoticeInput } from "./notices.ts";

/** Avviso di persistenza del backend (`store_warnings`, evento `store-warning`). */
export interface StoreWarning {
  file: string;
  kind: "corrupt" | "newer_version" | "unreadable" | "partially_read";
  message: string;
  /** Nome del backup datato, se il file è stato messo da parte. */
  backup: string | null;
  /** Unix ms. */
  at: number;
}

/** Avviso per l'utente: il testo è già in italiano dal backend; resta finché non lo chiude. */
export function storeWarningNotice(warning: StoreWarning): NoticeInput {
  return {
    key: `store:${warning.file}:${warning.at}`,
    level: "warning",
    text: warning.message,
    sticky: true,
  };
}
