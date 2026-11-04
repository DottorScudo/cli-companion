import { useState } from "react";
import type { KeyboardEvent } from "react";
import { Send, X } from "lucide-react";
import { sendMessage, startReply } from "../../app/actions/hub";
import { useStore } from "../../hooks/useStore";
import { errorMessage } from "../../lib/errors";
import { MESSAGE_MAX_CHARS, authorLabel, checkBody, excerpt, replyTarget } from "../../lib/hub";
import type { HubThread } from "../../lib/tauri";
import { hubStore } from "../../stores/hub";
import { Button } from "../ui/button";

/** Da quanti caratteri si mostra il conteggio. */
const COUNTER_FROM_CHARS = MESSAGE_MAX_CHARS - 500;

interface Props {
  channel: string;
  thread: HubThread;
}

/** Compositore in fondo alla conversazione: Invio invia, Maiusc+Invio va a capo. */
export default function HubComposer({ channel, thread }: Props) {
  const replyTo = useStore(hubStore, (s) => s.replyTo);
  const target = replyTarget(thread, replyTo);
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const send = async () => {
    const checked = checkBody(text);
    if (!checked.ok) {
      setError(checked.reason);
      return;
    }
    setError(null);
    setSending(true);
    try {
      await sendMessage(channel, thread.id, checked.body, target?.id ?? null);
      setText("");
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSending(false);
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    // Durante la composizione (IME) Invio conferma i caratteri, non invia.
    if (e.key !== "Enter" || e.shiftKey || e.nativeEvent.isComposing) return;
    e.preventDefault();
    if (!sending) void send();
  };

  const length = [...text].length;

  return (
    <div className="flex flex-col gap-1.5 border-t bg-base p-3">
      {target && (
        <div className="flex items-center gap-2 rounded-md border bg-card px-2 py-1 text-[11px] text-muted-foreground">
          <span className="min-w-0 flex-1 truncate">
            Risposta a <span className="font-medium">{authorLabel(target.author)}</span>: {excerpt(target.body)}
          </span>
          <button
            type="button"
            onClick={() => startReply(null)}
            aria-label="Annulla la risposta"
            title="Annulla la risposta"
            className="flex-shrink-0 rounded p-0.5 hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
          >
            <X className="!size-3.5" />
          </button>
        </div>
      )}
      <div className="flex items-end gap-2">
        <textarea
          value={text}
          rows={2}
          maxLength={MESSAGE_MAX_CHARS}
          disabled={sending}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder="Scrivi un messaggio (Invio invia, Maiusc+Invio va a capo)"
          aria-label="Messaggio"
          className="max-h-40 min-h-[3.25rem] flex-1 resize-y rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
        />
        <Button
          type="button"
          size="icon"
          onClick={() => void send()}
          disabled={sending || text.trim() === ""}
          aria-label="Invia"
          title="Invia"
        >
          <Send className="!size-4" />
        </Button>
      </div>
      <div className="flex items-center justify-between gap-2 text-[11px]">
        <span className="min-w-0 truncate text-destructive">{error}</span>
        {length >= COUNTER_FROM_CHARS && (
          <span className="flex-shrink-0 text-muted-foreground">
            {length}/{MESSAGE_MAX_CHARS}
          </span>
        )}
      </div>
    </div>
  );
}
