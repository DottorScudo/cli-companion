import { Star } from "lucide-react";
import { DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator } from "../ui/dropdown-menu";
import type { BuiltinLaunch, Launch, Workspace } from "../../lib/tauri";
import { resolveDefaultLaunch } from "../../lib/launch";
import { spawnTab } from "../../app/actions/tabs";
import { useStore } from "../../hooks/useStore";
import { configStore } from "../../stores/config";
import { MENU_ITEM } from "./styles";

/** Voci dei menu di avvio (⋯ e caret del pulsante diviso). */
export default function LaunchItems({ ws }: { ws: Workspace }) {
  const builtins = useStore(configStore, (s) => s.builtinLaunches);
  const globalLaunch = useStore(configStore, (s) => s.globalLaunch);
  const defId = resolveDefaultLaunch(ws, builtins, globalLaunch)?.id;
  const item = (l: Launch | BuiltinLaunch) => (
    <DropdownMenuItem key={l.id} title={l.command} onSelect={() => spawnTab(ws, l)} className={MENU_ITEM}>
      <span className="min-w-0 flex-1 truncate font-mono text-xs">{l.name}</span>
      {l.id === defId && (
        <span className="flex items-center gap-1 text-[10px] text-primary">
          <Star className="!size-3 fill-current" />
          predefinito
        </span>
      )}
    </DropdownMenuItem>
  );
  return (
    <>
      <DropdownMenuLabel className="px-2.5 pb-1 pt-1.5 text-[10px] font-semibold uppercase tracking-[0.08em] text-faint">
        Avvii
      </DropdownMenuLabel>
      {builtins.map(item)}
      {builtins.length > 0 && ws.launches.length > 0 && <DropdownMenuSeparator className="mx-1.5 bg-border" />}
      {ws.launches.map(item)}
      {builtins.length === 0 && ws.launches.length === 0 && (
        <DropdownMenuItem disabled className={MENU_ITEM}>
          Nessun avvio
        </DropdownMenuItem>
      )}
    </>
  );
}
