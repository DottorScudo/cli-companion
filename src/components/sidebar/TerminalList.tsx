import { memo } from "react";
import { Pin } from "lucide-react";
import StatusIcon from "../StatusIcon";
import BotAvatar from "../BotAvatar";
import { cn } from "../../lib/utils";
import { focusTerminal } from "../../app/actions/view";
import type { SidebarTerminal } from "../../stores/selectors";

/** Terminali aperti del workspace; un click porta il terminale in primo piano. */
function TerminalList({ terminals }: { terminals: SidebarTerminal[] }) {
  if (terminals.length === 0) return null;
  return (
    <ul className="mt-0.5 flex flex-col">
      {terminals.map((t) => (
        <li key={t.key} className="mx-1.5">
          <button
            type="button"
            // Un terminale scelto dalla sidebar riporta ai terminali del suo workspace.
            onClick={() => focusTerminal(t.key)}
            title={t.bot ? `${t.bot} · ${t.label}` : t.label}
            className={cn(
              "flex w-full items-center gap-2 rounded-md py-1.5 pl-3 pr-2 text-left transition-colors hover:bg-surface-2",
              t.active ? "bg-surface-2 text-foreground" : "text-text-secondary",
            )}
          >
            <span className="flex w-3.5 flex-shrink-0 justify-center">
              <StatusIcon status={t.status} />
            </span>
            {t.bot && <BotAvatar name={t.bot} size={16} image={t.botImage} />}
            <span className="min-w-0 flex-1 truncate font-mono text-xs">
              {t.bot ? `${t.bot} · ${t.label}` : t.label}
            </span>
            {t.pinned && (
              <Pin role="img" aria-label="Fissato nella vista globale" className="h-3 w-3 flex-shrink-0 text-primary" />
            )}
          </button>
        </li>
      ))}
    </ul>
  );
}

export default memo(TerminalList);
