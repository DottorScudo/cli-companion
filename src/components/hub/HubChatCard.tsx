import { memo } from "react";
import { MessageCircle } from "lucide-react";
import { openChat } from "../../app/actions/hubMessenger";
import { formatRelative } from "../../lib/board";
import { chatCardLabel, previewLine } from "../../lib/hubChats";
import type { ChatEntry, ThreadPreview } from "../../lib/hubChats";
import { findSecretary } from "../../lib/hubSecretaries";
import type { HubSecretary } from "../../lib/tauri";
import { cn } from "../../lib/utils";
import BotAvatar from "../BotAvatar";

const AVATAR_SIZE = 32;

interface Props {
  entry: ChatEntry;
  /** `undefined` finché l'anteprima non è stata letta. */
  preview: ThreadPreview | undefined;
  unread: boolean;
  selected: boolean;
  now: number;
  secretaries: readonly HubSecretary[];
}

function OwnerAvatar({ owner, secretaries }: { owner: string | null; secretaries: readonly HubSecretary[] }) {
  if (owner === null) {
    return (
      <span
        aria-hidden="true"
        className="flex flex-shrink-0 items-center justify-center rounded-full bg-surface-2 text-muted-foreground"
        style={{ width: AVATAR_SIZE, height: AVATAR_SIZE }}
      >
        <MessageCircle className="!size-4" />
      </span>
    );
  }
  const image = findSecretary(secretaries, owner)?.image ?? undefined;
  return <BotAvatar name={owner} size={AVATAR_SIZE} image={image} />;
}

/** Una chat dell'elenco messenger: avatar del titolare, titolo, workspace, ora, anteprima, «aspetta te» e non letti. */
function HubChatCard({ entry, preview, unread, selected, now, secretaries }: Props) {
  const { thread, channelName } = entry;
  const closed = thread.state !== "open";
  return (
    <li>
      <button
        type="button"
        onClick={() => openChat(thread.channel, thread.id)}
        aria-current={selected ? "true" : undefined}
        aria-label={chatCardLabel(entry, unread)}
        className={cn(
          "flex w-full items-start gap-2.5 border-b px-3 py-2.5 text-left transition-colors",
          selected ? "bg-accent" : "hover:bg-accent/50",
          closed && "opacity-60",
        )}
      >
        <OwnerAvatar owner={thread.owner} secretaries={secretaries} />
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="flex items-center gap-2">
            <span className={cn("min-w-0 flex-1 truncate text-[13px]", unread ? "font-semibold" : "font-medium")}>
              {thread.title}
            </span>
            <span className="flex-shrink-0 text-[11px] text-muted-foreground">
              {formatRelative(thread.updated_at, now)}
            </span>
          </span>
          <span className="truncate text-[11px] text-muted-foreground">{channelName}</span>
          <span className="flex items-center gap-2">
            <span className="min-w-0 flex-1 truncate text-[12px] text-muted-foreground">{previewLine(preview)}</span>
            {thread.waiting_user && (
              <span
                className="h-2 w-2 flex-shrink-0 rounded-full bg-status-attention"
                role="img"
                aria-label="Aspetta una tua risposta"
                title="Aspetta una tua risposta"
              />
            )}
            {unread && <span aria-hidden="true" className="h-2 w-2 flex-shrink-0 rounded-full bg-primary" />}
          </span>
        </span>
      </button>
    </li>
  );
}

export default memo(HubChatCard);
