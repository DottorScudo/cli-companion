import { useState } from "react";
import { Archive, ArchiveRestore, Loader2 } from "lucide-react";
import { setThreadClosed } from "../../app/actions/hub";
import { report } from "../../lib/errors";
import { threadStateAction } from "../../lib/hub";
import type { HubThread } from "../../lib/tauri";
import { Button } from "../ui/button";

/** «Chiudi thread» / «Riapri thread» nell'intestazione: lo stato lo cambia l'utente, e persiste al riavvio. */
export default function ThreadStateButton({ thread }: { thread: Pick<HubThread, "channel" | "id" | "state"> }) {
  const [pending, setPending] = useState(false);
  const action = threadStateAction(thread.state);
  const Icon = action.closed ? Archive : ArchiveRestore;

  async function change() {
    setPending(true);
    try {
      await setThreadClosed(thread.channel, thread.id, action.closed);
    } catch (e) {
      report(action.label.toLowerCase(), e, { notify: true });
    } finally {
      setPending(false);
    }
  }

  return (
    <Button
      type="button"
      size="sm"
      variant="outline"
      className="h-7 gap-1 px-2"
      disabled={pending}
      onClick={() => void change()}
      title={action.hint}
    >
      {pending ? (
        <Loader2 className="!size-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />
      ) : (
        <Icon className="!size-3.5" aria-hidden="true" />
      )}
      {action.label}
    </Button>
  );
}
