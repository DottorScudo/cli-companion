import { Pencil } from "lucide-react";
import { botSummary } from "../../../lib/botEditor";
import type { AgentBot, BotSkillOption, IntegrationInfo } from "../../../lib/tauri";
import BotAvatar from "../../BotAvatar";
import { cn } from "../../../lib/utils";

const AVATAR_SIZE = 24;

interface Props {
  bot: AgentBot;
  integrations: readonly IntegrationInfo[];
  skills: readonly BotSkillOption[];
  /** L'editor di questo Bot è aperto. */
  editing: boolean;
  disabled: boolean;
  onEdit: () => void;
}

/** Riga dell'elenco: avatar, nome, compito e riepilogo di agente, modello, effort e skill. */
export default function AgentBotRow({ bot, integrations, skills, editing, disabled, onEdit }: Props) {
  const summary = botSummary(bot, integrations, skills);
  return (
    <div
      className={cn(
        "flex min-w-0 items-center gap-3 rounded-md border bg-background/40 px-2 py-1.5",
        !bot.enabled && "opacity-60",
      )}
    >
      <BotAvatar name={bot.name} size={AVATAR_SIZE} image={bot.image ?? undefined} />
      <div className="min-w-0 flex-1">
        <p className="flex items-center gap-2 text-[13px] font-medium">
          <span className="truncate" title={bot.name}>
            {bot.name}
          </span>
          {!bot.enabled && <span className="rounded border px-1 py-px text-[10px] font-normal">disattivato</span>}
        </p>
        <p className="truncate text-[11px] text-muted-foreground" title={bot.task}>
          {bot.task}
        </p>
        <p className="truncate text-[11px] text-muted-foreground" title={summary.join(" · ")}>
          {summary.join(" · ")}
        </p>
      </div>
      <button
        type="button"
        disabled={disabled || editing}
        onClick={onEdit}
        title={`Modifica ${bot.name}`}
        aria-label={`Modifica ${bot.name}`}
        className="flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-[5px] text-muted-foreground transition-colors hover:bg-surface-2 hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:opacity-50"
      >
        <Pencil className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}
