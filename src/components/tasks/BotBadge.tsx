import BotAvatar from "../BotAvatar";
import { botById, botName } from "../../lib/taskBot";
import type { AgentBot } from "../../bindings/AgentBot";
import { cn } from "../../lib/utils";

const AVATAR_SIZE = 14;

interface Props {
  bots: readonly AgentBot[];
  /** Id del Bot del task; assente (task vecchi) = nessun badge. */
  id: string | undefined;
  className?: string;
}

/** Badge con avatar e nome del Bot che esegue il task; niente se il task non ha un Bot. */
export default function BotBadge({ bots, id, className }: Props) {
  const name = botName(bots, id);
  if (!name) return null;
  return (
    <span
      className={cn(
        "inline-flex min-w-0 max-w-[10rem] flex-shrink-0 items-center gap-1 rounded bg-surface-2 px-1.5 py-px font-medium text-text-secondary",
        className,
      )}
      title={`Bot: ${name}`}
    >
      <BotAvatar name={name} size={AVATAR_SIZE} image={botById(bots, id)?.image ?? undefined} />
      <span className="truncate">{name}</span>
    </span>
  );
}
