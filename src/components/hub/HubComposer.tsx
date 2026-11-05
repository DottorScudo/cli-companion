import { useId, useMemo, useRef, useState } from "react";
import type { ClipboardEvent, KeyboardEvent } from "react";
import { Paperclip, Send, X } from "lucide-react";
import { sendMessage, startReply } from "../../app/actions/hub";
import { pickAttachmentPaths } from "../../app/actions/hubAttachments";
import { useStore } from "../../hooks/useStore";
import { errorMessage } from "../../lib/errors";
import { MESSAGE_MAX_CHARS, authorLabel, checkBody, excerpt, replyTarget } from "../../lib/hub";
import { UPLOADING_TEXT, canSend, isUploading, pasteAttaches, readyAttachmentIds } from "../../lib/hubAttachments";
import { mentionable } from "../../lib/hubSecretaries";
import type { HubThread } from "../../lib/tauri";
import { cn } from "../../lib/utils";
import { hubStore } from "../../stores/hub";
import { Button } from "../ui/button";
import AttachmentDraftChip from "./AttachmentDraftChip";
import MentionMenu, { mentionOptionId } from "./MentionMenu";
import { useAttachmentDrafts } from "./useAttachmentDrafts";
import { useFileDrop } from "./useFileDrop";
import { useMentionAutocomplete } from "./useMentionAutocomplete";

/** Da quanti caratteri si mostra il conteggio. */
const COUNTER_FROM_CHARS = MESSAGE_MAX_CHARS - 500;

interface Props {
  channel: string;
  thread: HubThread;
}

/**
 * Compositore in fondo alla conversazione: Invio invia, Maiusc+Invio va a capo,
 * `@` chiama un segretario. Allegati dal pulsante (dialog), incollando file o
 * immagini e trascinando file sul compositore.
 */
export default function HubComposer({ channel, thread }: Props) {
  const replyTo = useStore(hubStore, (s) => s.replyTo);
  const target = replyTarget(thread, replyTo);
  const secretaries = useStore(hubStore, (s) => s.settings?.secretaries);
  const candidates = useMemo(() => mentionable(secretaries ?? [], channel), [secretaries, channel]);
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const menuId = useId();
  const mentions = useMentionAutocomplete({ text, setText, candidates, textareaRef });
  const attachments = useAttachmentDrafts(setError);
  const drop = useFileDrop(attachments.addFiles, !sending);
  const { drafts } = attachments;
  const uploading = drafts.some(isUploading);

  const send = async () => {
    if (uploading) {
      setError(UPLOADING_TEXT);
      return;
    }
    const ids = readyAttachmentIds(drafts);
    const checked = checkBody(text, ids.length);
    if (!checked.ok) {
      setError(checked.reason);
      return;
    }
    setError(null);
    setSending(true);
    try {
      await sendMessage(channel, thread.id, checked.body, target?.id ?? null, ids);
      setText("");
      attachments.clear();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSending(false);
    }
  };

  const browse = async () => {
    try {
      attachments.addPaths(await pickAttachmentPaths());
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (mentions.handleKeyDown(e)) return;
    // Durante la composizione (IME) Invio conferma i caratteri, non invia.
    if (e.key !== "Enter" || e.shiftKey || e.nativeEvent.isComposing) return;
    e.preventDefault();
    if (!sending) void send();
  };

  // Solo file (screenshot, file copiati): con del testo negli appunti si incolla il testo, come prima.
  const onPaste = (e: ClipboardEvent<HTMLTextAreaElement>) => {
    const { files, types } = e.clipboardData;
    if (!pasteAttaches(types, files.length)) return;
    e.preventDefault();
    attachments.addFiles(Array.from(files));
  };

  const length = [...text].length;

  return (
    <div
      {...drop.handlers}
      className={cn(
        "relative flex flex-col gap-1.5 border-t bg-base p-3",
        drop.dragging && "outline-dashed outline-2 -outline-offset-4 outline-primary",
      )}
    >
      {drop.dragging && (
        <p className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center bg-base/80 text-xs font-medium">
          Rilascia i file per allegarli
        </p>
      )}
      {mentions.menu && (
        <MentionMenu
          id={menuId}
          options={mentions.menu.options}
          active={mentions.menu.active}
          onSelect={mentions.complete}
        />
      )}
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
      {drafts.length > 0 && (
        <ul aria-label="Allegati da inviare" className="flex flex-wrap gap-1.5">
          {drafts.map((d) => (
            <AttachmentDraftChip key={d.key} draft={d} onRemove={attachments.remove} />
          ))}
        </ul>
      )}
      <div className="flex items-end gap-2">
        <textarea
          ref={textareaRef}
          value={text}
          rows={2}
          maxLength={MESSAGE_MAX_CHARS}
          disabled={sending}
          onChange={(e) => {
            setText(e.target.value);
            mentions.track(e.target.value, e.target.selectionStart);
          }}
          onSelect={(e) => mentions.track(e.currentTarget.value, e.currentTarget.selectionStart)}
          onKeyDown={onKeyDown}
          onPaste={onPaste}
          placeholder="Scrivi un messaggio, @ per chiamare un segretario (Invio invia, Maiusc+Invio va a capo)"
          aria-label="Messaggio"
          role="combobox"
          aria-autocomplete="list"
          aria-expanded={mentions.menu !== null}
          aria-controls={mentions.menu ? menuId : undefined}
          aria-activedescendant={mentions.menu ? mentionOptionId(menuId, mentions.menu.active) : undefined}
          className="max-h-40 min-h-[3.25rem] flex-1 resize-y rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
        />
        <Button
          type="button"
          size="icon"
          variant="outline"
          onClick={() => void browse()}
          disabled={sending}
          aria-label="Allega"
          title="Allega file (anche incollando o trascinando)"
        >
          <Paperclip className="!size-4" />
        </Button>
        <Button
          type="button"
          size="icon"
          onClick={() => void send()}
          disabled={!canSend(text, drafts, sending)}
          aria-label="Invia"
          title={uploading ? UPLOADING_TEXT : "Invia"}
        >
          <Send className="!size-4" />
        </Button>
      </div>
      <div className="flex items-center justify-between gap-2 text-[11px]">
        <span className="min-w-0 truncate text-destructive" title={error ?? undefined}>
          {error}
        </span>
        {length >= COUNTER_FROM_CHARS && (
          <span className="flex-shrink-0 text-muted-foreground">
            {length}/{MESSAGE_MAX_CHARS}
          </span>
        )}
      </div>
    </div>
  );
}
