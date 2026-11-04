export type NoticeLevel = "error" | "warning";

/** Avviso da mostrare all'utente. `key` identifica lo stesso avviso ripetuto. */
export interface NoticeInput {
  key: string;
  level: NoticeLevel;
  text: string;
  /** Resta finché l'utente non lo chiude (gli altri scompaiono da soli). */
  sticky?: boolean;
}

export interface Notice extends NoticeInput {
  /** Cambia a ogni inserimento: serve a riarmare il timer di scomparsa. */
  id: number;
  sticky: boolean;
}

/** Avvisi visibili insieme: oltre il tetto cadono i più vecchi. */
export const MAX_NOTICES = 5;
/** Dopo quanto scompare da solo un avviso non sticky. */
export const NOTICE_TTL_MS = 8000;

/** Aggiunge un avviso; uno con la stessa chiave sostituisce il precedente e torna in fondo. */
export function addNotice(list: readonly Notice[], input: NoticeInput, id: number): Notice[] {
  const next = list.filter((n) => n.key !== input.key);
  next.push({ ...input, sticky: input.sticky === true, id });
  return next.slice(-MAX_NOTICES);
}

export function removeNotice(list: readonly Notice[], id: number): Notice[] {
  return list.filter((n) => n.id !== id);
}

/** Archivio degli avvisi, osservabile con `useSyncExternalStore`. */
export function createNoticeStore() {
  let notices: Notice[] = [];
  let nextId = 1;
  const listeners = new Set<() => void>();
  const update = (next: Notice[]) => {
    notices = next;
    listeners.forEach((l) => l());
  };
  return {
    push(input: NoticeInput) {
      update(addNotice(notices, input, nextId++));
    },
    dismiss(id: number) {
      const next = removeNotice(notices, id);
      if (next.length !== notices.length) update(next);
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
    getSnapshot: (): readonly Notice[] => notices,
  };
}

export const noticeStore = createNoticeStore();
