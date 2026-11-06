import { memo } from "react";
import { ChevronDown, Plus } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "../ui/dropdown-menu";
import type { BotLaunch, BuiltinLaunch, Launch, Workspace } from "../../lib/tauri";
import { spawnTab } from "../../app/actions/tabs";
import LaunchItems from "./LaunchItems";
import { manageLaunches } from "./actions";
import { MENU_CONTENT, MENU_ITEM } from "./styles";

interface Props {
  ws: Workspace;
  /** Avvio predefinito del workspace (undefined = shell vuota). */
  def: Launch | BuiltinLaunch | BotLaunch | undefined;
}

/** Pulsante diviso: avvio predefinito + scelta dell'avvio. */
function SpawnSplitButton({ ws, def }: Props) {
  return (
    <div className="mx-1.5 mt-1.5 flex gap-1">
      <button
        type="button"
        title={def ? `Esegue: ${def.name}` : "Apre una shell vuota"}
        onClick={() => spawnTab(ws)}
        className="flex min-w-0 flex-1 items-center gap-2 rounded-md border border-dashed border-border-strong px-2.5 py-1.5 text-muted-foreground transition-colors hover:border-muted-foreground hover:text-foreground"
      >
        <Plus className="h-3 w-3 flex-shrink-0" />
        <span className="text-xs">Terminale</span>
        {def && (
          <span className="ml-auto min-w-0 truncate font-mono text-[11px] text-muted-foreground">{def.name}</span>
        )}
      </button>
      <DropdownMenu modal={false}>
        <DropdownMenuTrigger asChild>
          <button
            type="button"
            aria-label="Scegli un avvio"
            title="Scegli un avvio"
            className="flex w-[30px] flex-shrink-0 items-center justify-center rounded-md border border-dashed border-border-strong text-muted-foreground transition-colors hover:border-muted-foreground hover:text-foreground"
          >
            <ChevronDown className="h-3 w-3" />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className={MENU_CONTENT} onCloseAutoFocus={(e) => e.preventDefault()}>
          <LaunchItems ws={ws} />
          <DropdownMenuSeparator className="mx-1.5 bg-border" />
          <DropdownMenuItem onSelect={() => manageLaunches(ws)} className={MENU_ITEM}>
            Gestisci avvii…
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}

export default memo(SpawnSplitButton);
