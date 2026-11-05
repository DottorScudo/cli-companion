import { AlertTriangle } from "lucide-react";
import { formatRelative } from "../../lib/board";
import type { HubEvent, HubMessage } from "../../lib/tauri";
import ReplyReference from "./ReplyReference";
import { useEventLinks } from "./useEventLinks";
import WorkerTerminalLink from "./WorkerTerminalLink";

interface Props {
  message: HubMessage;
  /** L'evento della domanda a cui la risposta non è arrivata. */
  question: HubEvent;
  target: HubMessage | null;
  now: number;
}

/** Esito «Risposta non inviata» come avviso in evidenza, con il link al terminale del worker (se il tab è aperto) per rispondere lì. */
export default function HubDeliveryFailure({ message, question, target, now }: Props) {
  const { workerTab } = useEventLinks(question);
  return (
    <article
      role="alert"
      className="flex min-w-0 flex-col gap-1 rounded-lg border border-destructive/50 bg-destructive/5 px-3 py-2"
    >
      <header className="flex items-center gap-2">
        <AlertTriangle className="!size-3.5 flex-shrink-0 text-destructive" aria-hidden="true" />
        <h3 className="flex-1 text-[12px] font-semibold text-destructive">Risposta non consegnata</h3>
        <span className="flex-shrink-0 text-[11px] text-muted-foreground">
          {formatRelative(message.created_at, now)}
        </span>
      </header>
      <p className="whitespace-pre-line break-words text-[12px] text-text-secondary">{message.body}</p>
      {target && <ReplyReference target={target} />}
      {workerTab !== null && <WorkerTerminalLink tab={workerTab} />}
    </article>
  );
}
