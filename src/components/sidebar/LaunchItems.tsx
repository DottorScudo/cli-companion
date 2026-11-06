import { Star } from "lucide-react";
import {
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
} from "../ui/dropdown-menu";
import type { Launch, Workspace } from "../../lib/tauri";
import { resolveDefaultLaunch } from "../../lib/launch";
import { buildLaunchMenu } from "../../lib/launchMenu";
import type { LaunchMenuBotEntry, LaunchMenuEntry } from "../../lib/launchMenu";
import { spawnBotFromMenu, spawnFromMenu } from "../../app/actions/tabs";
import { useStore } from "../../hooks/useStore";
import { botLaunchesFor, configStore, launchesFor } from "../../stores/config";
import { cn } from "../../lib/utils";
import { MENU_CONTENT, MENU_ITEM } from "./styles";

const GROUP_LABEL = "px-2.5 pb-1 pt-1.5 text-[10px] font-semibold uppercase tracking-[0.08em] text-faint";

/** Voci dei menu di avvio (⋯ e caret del pulsante diviso). */
export default function LaunchItems({ ws }: { ws: Workspace }) {
  const builtins = useStore(configStore, (s) => launchesFor(s, ws));
  const botLaunches = useStore(configStore, (s) => botLaunchesFor(s, ws));
  const integrations = useStore(configStore, (s) => s.integrations);
  const globalLaunch = useStore(configStore, (s) => s.globalLaunch);
  const defId = resolveDefaultLaunch(ws, builtins, globalLaunch)?.id;
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
      {menu.others.length > 0 && (
        <DropdownMenuSub>
          <DropdownMenuSubTrigger className={MENU_ITEM}>Avvia con…</DropdownMenuSubTrigger>
          <DropdownMenuSubContent className={MENU_CONTENT}>
            {menu.others.map((agent) => (
              <div key={agent.agentId}>
                <DropdownMenuLabel className={GROUP_LABEL}>{agent.agentName}</DropdownMenuLabel>
                {agent.entries.map(builtinItem)}
              </div>
            ))}
          </DropdownMenuSubContent>
        </DropdownMenuSub>
      )}
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
