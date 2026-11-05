import { FileX, Loader2 } from "lucide-react";
import { openAttachmentPath } from "../../app/actions/hubAttachments";
import { report } from "../../lib/errors";
import { formatSize, hasPreview } from "../../lib/hubAttachments";
import type { HubAttachment } from "../../lib/tauri";
import AttachmentIcon from "./AttachmentIcon";
import { useStoredAttachment } from "./useStoredAttachment";

const UNAVAILABLE = "non disponibile";

interface Props {
  channel: string;
  threadId: string;
  attachments: readonly HubAttachment[];
}

function openStored(path: string): void {
  openAttachmentPath(path).catch((e) => report("apertura dell'allegato", e, { notify: true }));
}

const CHIP =
  "flex max-w-full items-center gap-1.5 rounded-md border bg-card px-2 py-1 text-left text-[11px] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring";

interface ItemProps {
  channel: string;
  threadId: string;
  attachment: HubAttachment;
}

/** Un allegato della bolla: miniatura per le immagini, chip per gli altri file; cliccandolo si apre con l'app predefinita. */
function StoredAttachmentItem({ channel, threadId, attachment }: ItemProps) {
  const stored = useStoredAttachment(channel, threadId, attachment.id, attachment.mime);
  const size = formatSize(attachment.size);

  if (stored.status === "missing") {
    return (
      <li
        aria-label={`${attachment.name}, ${UNAVAILABLE}`}
        title={`${attachment.name}: il file non esiste più`}
        className={`${CHIP} text-muted-foreground`}
      >
        <FileX aria-hidden className="!size-4 flex-shrink-0" />
        <span className="min-w-0 truncate line-through">{attachment.name}</span>
        <span className="flex-shrink-0 italic">{UNAVAILABLE}</span>
      </li>
    );
  }

  const ready = stored.status === "ready" ? stored : null;
  const label = `Apri ${attachment.name}, ${size}`;
  if (hasPreview(attachment.mime)) {
    return (
      <li>
        <button
          type="button"
          disabled={!ready}
          onClick={() => ready && openStored(ready.path)}
          aria-label={label}
          title={label}
          className="block overflow-hidden rounded-md border bg-card focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
        >
          {ready?.preview ? (
            <img src={ready.preview} alt={attachment.name} className="max-h-40 max-w-[12rem] object-contain" />
          ) : (
            <span className="flex size-20 items-center justify-center text-muted-foreground">
              <Loader2 aria-hidden className="!size-4 animate-spin motion-reduce:animate-none" />
            </span>
          )}
        </button>
      </li>
    );
  }
  return (
    <li className="min-w-0">
      <button
        type="button"
        disabled={!ready}
        onClick={() => ready && openStored(ready.path)}
        aria-label={label}
        title={label}
        className={`${CHIP} hover:bg-accent disabled:opacity-60`}
      >
        <AttachmentIcon mime={attachment.mime} className="!size-4 flex-shrink-0 text-muted-foreground" />
        <span className="min-w-0 truncate">{attachment.name}</span>
        <span className="flex-shrink-0 text-muted-foreground">{size}</span>
      </button>
    </li>
  );
}

/** Gli allegati di un messaggio dell'utente, sotto il testo. */
export default function HubMessageAttachments({ channel, threadId, attachments }: Props) {
  if (attachments.length === 0) return null;
  return (
    <ul aria-label="Allegati" className="flex flex-wrap gap-1.5">
      {attachments.map((a) => (
        <StoredAttachmentItem key={a.id} channel={channel} threadId={threadId} attachment={a} />
      ))}
    </ul>
  );
}
