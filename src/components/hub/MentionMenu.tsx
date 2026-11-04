import type { HubSecretary } from "../../lib/tauri";
import { cn } from "../../lib/utils";
import BotAvatar from "../BotAvatar";
import { promptFirstLine } from "../../lib/hubSecretaries";

const AVATAR_SIZE = 20;

interface Props {
  /** Id del listbox, per `aria-controls` del campo di testo. */
  id: string;
  options: readonly HubSecretary[];
  active: number;
  onSelect: (secretary: HubSecretary) => void;
}

/** Id dell'opzione `index`, per `aria-activedescendant`. */
export const mentionOptionId = (menuId: string, index: number): string => `${menuId}-option-${index}`;

/** Elenco dei segretari che si possono chiamare, sopra il compositore; il focus resta nel campo di testo. */
export default function MentionMenu({ id, options, active, onSelect }: Props) {
  return (
    <ul
      id={id}
      role="listbox"
      aria-label="Segretari"
      className="absolute bottom-full left-3 z-10 mb-1 flex max-h-56 w-72 flex-col gap-0.5 overflow-y-auto rounded-md border bg-popover p-1 shadow-md"
    >
      {options.map((s, i) => (
        <li
          key={s.name}
          id={mentionOptionId(id, i)}
          role="option"
          aria-selected={i === active}
          // Il mouse non toglie il focus al campo di testo.
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => onSelect(s)}
          className={cn(
            "flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-[13px]",
            i === active ? "bg-surface text-foreground" : "text-text-secondary hover:bg-surface/60",
          )}
        >
          <BotAvatar name={s.name} size={AVATAR_SIZE} image={s.image ?? undefined} />
          <span className="font-mono font-medium">@{s.name}</span>
          <span className="min-w-0 flex-1 truncate text-[11px] text-muted-foreground">
            {promptFirstLine(s.system_prompt)}
          </span>
        </li>
      ))}
    </ul>
  );
}
