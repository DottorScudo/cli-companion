import { useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent, RefObject } from "react";
import { completeMention, filterMentions, mentionQuery, moveActive } from "../../lib/hubSecretaries";
import type { HubSecretary } from "../../lib/tauri";

interface Options {
  text: string;
  setText: (text: string) => void;
  /** Segretari che si possono chiamare nel canale. */
  candidates: readonly HubSecretary[];
  textareaRef: RefObject<HTMLTextAreaElement | null>;
}

/**
 * Autocompletamento di `@` nel compositore: il menu compare mentre si scrive
 * una menzione e ci sono segretari che corrispondono. Frecce scorrono, Invio
 * e Tab completano, Esc chiude solo il menu (finché non si scrive un'altra menzione).
 */
export function useMentionAutocomplete({ text, setText, candidates, textareaRef }: Options) {
  const [caret, setCaret] = useState(0);
  const [activeIndex, setActiveIndex] = useState(0);
  /** Posizione del `@` della menzione chiusa con Esc. */
  const [dismissedAt, setDismissedAt] = useState<number | null>(null);
  const pendingCaret = useRef<number | null>(null);

  const mention = mentionQuery(text, caret);
  const options = mention === null ? [] : filterMentions(candidates, mention.query);
  const open = mention !== null && options.length > 0 && dismissedAt !== mention.start;
  const active = open ? Math.min(activeIndex, options.length - 1) : -1;

  // Dopo il completamento il cursore va dopo il nome, quando il testo nuovo è nel campo.
  useLayoutEffect(() => {
    const el = textareaRef.current;
    if (pendingCaret.current === null || el === null) return;
    el.setSelectionRange(pendingCaret.current, pendingCaret.current);
    pendingCaret.current = null;
  }, [text, textareaRef]);

  const track = (nextText: string, nextCaret: number) => {
    setCaret(nextCaret);
    setActiveIndex(0);
    if (mentionQuery(nextText, nextCaret) === null) setDismissedAt(null);
  };

  const complete = (secretary: HubSecretary) => {
    if (mention === null) return;
    const done = completeMention(text, mention, caret, secretary.name);
    pendingCaret.current = done.caret;
    setText(done.text);
    track(done.text, done.caret);
  };

  /** Gestisce i tasti del menu; `true` se il tasto è stato consumato. */
  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>): boolean => {
    if (!open || mention === null || e.nativeEvent.isComposing) return false;
    switch (e.key) {
      case "ArrowDown":
      case "ArrowUp":
        e.preventDefault();
        setActiveIndex(moveActive(active, e.key === "ArrowDown" ? 1 : -1, options.length));
        return true;
      case "Enter":
      case "Tab":
        if (e.shiftKey) return false;
        e.preventDefault();
        complete(options[active]);
        return true;
      case "Escape":
        e.preventDefault();
        e.stopPropagation();
        setDismissedAt(mention.start);
        return true;
      default:
        return false;
    }
  };

  return {
    menu: open ? { options, active } : null,
    /** Il testo o il cursore sono cambiati. */
    track,
    complete,
    handleKeyDown,
  };
}
