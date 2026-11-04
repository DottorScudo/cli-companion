import { CornerUpLeft } from "lucide-react";
import { authorLabel, excerpt } from "../../lib/hub";
import type { HubMessage } from "../../lib/tauri";

/** Riferimento al messaggio originale di una risposta: autore e anteprima del testo. */
export default function ReplyReference({ target }: { target: HubMessage }) {
  return (
    <p className="flex min-w-0 items-center gap-1 text-[11px] text-muted-foreground">
      <CornerUpLeft className="!size-3 flex-shrink-0" aria-hidden="true" />
      <span className="flex-shrink-0 font-medium">{authorLabel(target.author)}</span>
      <span className="truncate">{excerpt(target.body)}</span>
    </p>
  );
}
