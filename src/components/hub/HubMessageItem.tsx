import { memo } from "react";
import { formatRelative } from "../../lib/board";
import { authorLabel } from "../../lib/hub";
import type { HubMessage } from "../../lib/tauri";
import Markdown from "../tasks/Markdown";
import HubEventRow from "./HubEventRow";
import ReplyReference from "./ReplyReference";

interface Props {
  message: HubMessage;
  /** Il messaggio a cui questo risponde, se esiste ancora. */
  target: HubMessage | null;
  now: number;
  onReply: (messageId: string) => void;
}

/** Un messaggio della conversazione: testo con autore e orario, oppure evento in forma compatta. */
function HubMessageItem({ message, target, now, onReply }: Props) {
  if (message.event) {
    return <HubEventRow message={message} event={message.event} target={target} now={now} onReply={onReply} />;
  }
  return (
    <article className="flex flex-col gap-1 rounded-lg border bg-card px-3 py-2">
      <header className="flex items-baseline gap-2">
        <span className="text-[12px] font-semibold">{authorLabel(message.author)}</span>
        <span className="text-[11px] text-muted-foreground">{formatRelative(message.created_at, now)}</span>
      </header>
      {target && <ReplyReference target={target} />}
      <Markdown source={message.body} />
    </article>
  );
}

export default memo(HubMessageItem);
