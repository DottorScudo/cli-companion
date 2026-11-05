import { useEffect, useState } from "react";
import { attachmentPath, attachmentPreviewUrl } from "../../app/actions/hubAttachments";
import { hasPreview } from "../../lib/hubAttachments";

export type StoredAttachment =
  { status: "loading" } | { status: "ready"; path: string; preview: string | null } | { status: "missing" };

/**
 * Il file di un allegato di un messaggio: il percorso (per aprirlo) e, per le
 * immagini, un Blob URL dell'anteprima, revocato allo smontaggio. Un file che
 * non c'è più (o non si legge) dà `missing`, senza errori per la conversazione.
 * Dipende da id e tipo, non dall'oggetto: una rilettura del thread non lo ricarica.
 */
export function useStoredAttachment(
  channel: string,
  threadId: string,
  attachmentId: string,
  mime: string,
): StoredAttachment {
  const [state, setState] = useState<StoredAttachment>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    let preview: string | null = null;
    const load = async () => {
      const path = await attachmentPath(channel, threadId, attachmentId);
      const url = hasPreview(mime) ? await attachmentPreviewUrl(channel, threadId, attachmentId, mime) : null;
      if (cancelled) {
        if (url) URL.revokeObjectURL(url);
        return;
      }
      preview = url;
      setState({ status: "ready", path, preview: url });
    };
    setState({ status: "loading" });
    load().catch(() => {
      if (!cancelled) setState({ status: "missing" });
    });
    return () => {
      cancelled = true;
      if (preview) URL.revokeObjectURL(preview);
    };
  }, [channel, threadId, attachmentId, mime]);

  return state;
}
