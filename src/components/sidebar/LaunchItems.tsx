import { Star } from "lucide-react";
import { DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator } from "../ui/dropdown-menu";
import type { Launch, Workspace } from "../../lib/tauri";
import { buildLaunchMenu } from "../../lib/launchMenu";
import type { LaunchMenuBotEntry, LaunchMenuEntry } from "../../lib/launchMenu";
import { spawnBotFromMenu, spawnFromMenu } from "../../app/actions/tabs";
import { useStore } from "../../hooks/useStore";
import { botLaunchesFor, configStore, defaultLaunchFor } from "../../stores/config";
import { cn } from "../../lib/utils";
import { MENU_ITEM } from "./styles";

const GROUP_LABEL = "px-2.5 pb-1 pt-1.5 text-[10px] font-semibold uppercase tracking-[0.08em] text-faint";

/** Voci dei menu di avvio (⋯ e caret del pulsante diviso). */
export default function LaunchItems({ ws }: { ws: Workspace }) {
  const builtins = useStore(configStore, (s) => s.builtinLaunches);
  const botLaunches = useStore(configStore, (s) => botLaunchesFor(s, ws));
  const integrations = useStore(configStore, (s) => s.integrations);
  const defId = useStore(configStore, (s) => defaultLaunchFor(s, ws)?.id);
  const menu = buildLaunchMenu(builtins, integrations, ws, botLaunches);

  const defaultTag = (id: string) =>
    id === defId && (
      <span className="flex items-center gap-1 text-[10px] text-primary">
        <Star className="!size-3 fill-current" />
        predefinito
      </span>
    );

  const builtinItem = ({ launch, label, blockedReason }: LaunchMenuEntry) => (
    <DropdownMenuItem
      key={launch.id}
      title={blockedReason ?? launch.command}
      onSelect={() => spawnFromMenu(ws, launch)}
      className={cn(MENU_ITEM, blockedReason !== null && "opacity-60")}
    >
      <span className="min-w-0 flex-1 truncate font-mono text-xs">{label}</span>
      {blockedReason !== null && <span className="text-[10px] text-faint">non pronto</span>}
      {defaultTag(launch.id)}
    </DropdownMenuItem>
  );

  const botItem = ({ launch, label, blockedReason }: LaunchMenuBotEntry) => (
    <DropdownMenuItem
      key={launch.id}
      title={blockedReason ?? launch.name}
      onSelect={() => void spawnBotFromMenu(ws, launch)}
      className={cn(MENU_ITEM, blockedReason !== null && "opacity-60")}
    >
      <span className="min-w-0 flex-1 truncate font-mono text-xs">{label}</span>
      {blockedReason !== null && <span className="text-[10px] text-faint">non pronto</span>}
      {defaultTag(launch.id)}
    </DropdownMenuItem>
  );

  const customItem = (l: Launch) => (
    <DropdownMenuItem key={l.id} title={l.command} onSelect={() => spawnFromMenu(ws, l)} className={MENU_ITEM}>
      <span className="min-w-0 flex-1 truncate font-mono text-xs">{l.name}</span>
      {defaultTag(l.id)}
    </DropdownMenuItem>
  );

  return (
    <>
      <DropdownMenuLabel className={GROUP_LABEL}>Avvii</DropdownMenuLabel>
      {menu.entries.map(builtinItem)}
      {menu.bots.length > 0 && (
        <>
          <DropdownMenuSeparator className="mx-1.5 bg-border" />
          <DropdownMenuLabel className={GROUP_LABEL}>Bot</DropdownMenuLabel>
          {menu.bots.map(botItem)}
        </>
      )}
      {menu.entries.length > 0 && ws.launches.length > 0 && <DropdownMenuSeparator className="mx-1.5 bg-border" />}
      {ws.launches.map(customItem)}
      {menu.entries.length === 0 && menu.bots.length === 0 && ws.launches.length === 0 && (
        <DropdownMenuItem disabled className={MENU_ITEM}>
          Nessun avvio
        </DropdownMenuItem>
      )}
    </>
  );
}
