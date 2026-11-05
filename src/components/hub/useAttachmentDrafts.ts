import { useCallback, useEffect, useRef, useState } from "react";
import { stageAttachmentBlob, stageAttachmentFile } from "../../app/actions/hubAttachments";
import { errorMessage } from "../../lib/errors";
import { admitFiles, baseName, draftFileName, draftStaged, hasPreview } from "../../lib/hubAttachments";
import type { AttachmentDraft, Candidate } from "../../lib/hubAttachments";
import type { HubAttachment } from "../../lib/tauri";

interface Pending extends Candidate {
  /** La miniatura, creata solo per i file ammessi. */
  preview: () => string | null;
  upload: () => Promise<HubAttachment>;
}

function revoke(draft: AttachmentDraft): void {
  if (draft.preview) URL.revokeObjectURL(draft.preview);
}

/**
 * Gli allegati in attesa del compositore: ammissione nei limiti, caricamento
 * nello staging (in parallelo), rimozione. Un file che non si carica esce dai
 * chip e l'errore va a `onError`, senza toccare il testo. Le miniature sono Blob
 * URL dei file incollati o trascinati, revocati alla rimozione e allo smontaggio.
 */
export function useAttachmentDrafts(onError: (message: string) => void) {
  const [drafts, setDrafts] = useState<AttachmentDraft[]>([]);
  // Copia sincrona dell'elenco: due aggiunte nello stesso tick vedono entrambe i posti già presi.
  const draftsRef = useRef<AttachmentDraft[]>([]);
  const nextKey = useRef(0);

  const update = useCallback((change: (list: AttachmentDraft[]) => AttachmentDraft[]) => {
    draftsRef.current = change(draftsRef.current);
    setDrafts(draftsRef.current);
  }, []);

  useEffect(() => () => draftsRef.current.forEach(revoke), []);

  const drop = useCallback(
    (key: string) => {
      const draft = draftsRef.current.find((d) => d.key === key);
      if (draft) revoke(draft);
      update((list) => list.filter((d) => d.key !== key));
    },
    [update],
  );

  const start = useCallback(
    (candidates: Pending[]) => {
      const { accepted, error } = admitFiles(draftsRef.current.length, candidates);
      if (error) onError(error);
      for (const c of accepted) {
        nextKey.current += 1;
        const key = `draft-${nextKey.current}`;
        update((list) => [...list, { key, name: c.name, size: c.size, preview: c.preview(), attachment: null }]);
        c.upload().then(
          (attachment) => update((list) => draftStaged(list, key, attachment)),
          (e: unknown) => {
            // Rimosso nel frattempo: nessun errore da mostrare.
            if (!draftsRef.current.some((d) => d.key === key)) return;
            drop(key);
            onError(`Caricamento di «${c.name}» non riuscito: ${errorMessage(e)}`);
          },
        );
      }
    },
    [drop, onError, update],
  );

  /** File senza percorso: incollati o trascinati. */
  const addFiles = useCallback(
    (files: readonly File[]) => {
      start(
        files.map((file) => {
          const name = draftFileName(file.name, file.type);
          return {
            name,
            size: file.size,
            preview: () => (hasPreview(file.type) ? URL.createObjectURL(file) : null),
            upload: () => stageAttachmentBlob(name, file),
          };
        }),
      );
    },
    [start],
  );

  /** File scelti con il dialog: la dimensione la controlla il backend al caricamento. */
  const addPaths = useCallback(
    (paths: readonly string[]) => {
      start(
        paths.map((path) => ({
          name: baseName(path),
          size: null,
          preview: () => null,
          upload: () => stageAttachmentFile(path),
        })),
      );
    },
    [start],
  );

  /** Dopo l'invio: i file sono passati al messaggio, i chip si svuotano. */
  const clear = useCallback(() => {
    draftsRef.current.forEach(revoke);
    update(() => []);
  }, [update]);

  return { drafts, addFiles, addPaths, remove: drop, clear };
}
