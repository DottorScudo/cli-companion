import { Loader2, X } from "lucide-react";
import { draftLabel, formatSize, isUploading } from "../../lib/hubAttachments";
import type { AttachmentDraft } from "../../lib/hubAttachments";
import AttachmentIcon from "./AttachmentIcon";

interface Props {
  draft: AttachmentDraft;
  onRemove: (key: string) => void;
}

/** Un allegato in attesa nel compositore: miniatura o icona, nome, dimensione, caricamento, X per toglierlo. */
export default function AttachmentDraftChip({ draft, onRemove }: Props) {
  const uploading = isUploading(draft);
  const label = draftLabel(draft);
  return (
    <li
      aria-label={label}
      aria-busy={uploading}
      title={label}
      className="flex max-w-[14rem] items-center gap-1.5 rounded-md border bg-card py-1 pl-1 pr-1.5 text-[11px]"
    >
      {draft.preview ? (
        <img src={draft.preview} alt="" className="size-7 flex-shrink-0 rounded object-cover" />
      ) : (
        <AttachmentIcon
          mime={draft.attachment?.mime ?? null}
          className="!size-4 mx-1.5 flex-shrink-0 text-muted-foreground"
        />
      )}
      <span className="min-w-0 flex-1 truncate">{draft.name}</span>
      {draft.size !== null && <span className="flex-shrink-0 text-muted-foreground">{formatSize(draft.size)}</span>}
      {uploading && (
        <Loader2
          aria-hidden
          className="!size-3 flex-shrink-0 animate-spin text-muted-foreground motion-reduce:animate-none"
        />
      )}
      <button
        type="button"
        onClick={() => onRemove(draft.key)}
        aria-label={`Rimuovi ${draft.name}`}
        title={`Rimuovi ${draft.name}`}
        className="flex-shrink-0 rounded p-0.5 text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
      >
        <X className="!size-3" />
      </button>
    </li>
  );
}
