import { useState } from "react";
import { Loader2, Trash2 } from "lucide-react";
import { deleteThread } from "../../app/actions/hub";
import { report } from "../../lib/errors";
import type { HubThread } from "../../lib/tauri";
import { Button } from "../ui/button";

const CONFIRM_TEXT =
  "Eliminare questo thread? Messaggi e allegati spariscono, i task restano nella board (se uno è ancora attivo, i suoi eventi apriranno un thread nuovo). Non si può annullare.";

/**
 * «Elimina thread» nell'intestazione, con conferma in linea. Va montato con
 * `key` sull'id del thread: cambiando thread la conferma si chiude.
 */
export default function DeleteThreadButton({ thread }: { thread: Pick<HubThread, "channel" | "id"> }) {
  const [confirming, setConfirming] = useState(false);
  const [pending, setPending] = useState(false);

  async function remove() {
    setPending(true);
    try {
      await deleteThread(thread.channel, thread.id);
    } catch (e) {
      report("eliminazione del thread", e, { notify: true });
      setPending(false);
    }
  }

  if (!confirming) {
    return (
      <Button
        type="button"
        size="sm"
        variant="outline"
        className="h-7 gap-1 px-2"
        onClick={() => setConfirming(true)}
        title="Elimina il thread con messaggi e allegati; i task restano nella board"
      >
        <Trash2 className="!size-3.5" aria-hidden="true" />
        Elimina thread
      </Button>
    );
  }

  return (
    <div role="group" aria-label="Conferma eliminazione del thread" className="flex min-w-0 items-center gap-2">
      <span className="min-w-0 max-w-sm text-[11px] leading-tight text-muted-foreground">{CONFIRM_TEXT}</span>
      <Button
        type="button"
        size="sm"
        variant="destructive"
        className="h-7 gap-1 px-2"
        disabled={pending}
        onClick={() => void remove()}
      >
        {pending && <Loader2 className="!size-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />}
        Elimina
      </Button>
      <Button
        type="button"
        size="sm"
        variant="outline"
        className="h-7 px-2"
        disabled={pending}
        onClick={() => setConfirming(false)}
      >
        Annulla
      </Button>
    </div>
  );
}
