// Derivazioni pure condivise dai componenti: etichette, stati aggregati, tab
// della barra del titolo e terminali della sidebar. Prendono lo stato degli
// archivi come argomenti, così si testano senza React.

import { GROUP_STATUS, countNeedingInput } from "../lib/agents.ts";
import type { TabStatus } from "../lib/agents.ts";
import type { BotImage } from "../lib/bots.ts";
import { GLOBAL_ID, groupIndicator } from "../lib/layout.ts";
import type { LayoutState } from "../lib/layout.ts";
import { allLaunches } from "../lib/launch.ts";
import type { BotSettings, BuiltinLaunch, Workspace } from "../lib/tauri.ts";
import { shownBot } from "./config.ts";
import { sameStatus, statusOf } from "./tabRuntime.ts";
import type { RuntimeState, TabRuntime } from "./tabRuntime.ts";
import type { TabEntry, TabsState } from "./tabs.ts";

/** Nome mostrato sul tab di raggruppamento della vista globale. */
const GLOBAL_TAB_NAME = "Globale";

/** Tab della barra del titolo (vista globale o gruppo di un workspace). */
export interface TitleTab {
  /** Id del workspace (o GLOBAL_ID per la vista globale). */
  id: string;
  name: string;
  count: number;
  /** Stato aggregato del gruppo (null = nessun indicatore). */
  status: TabStatus | null;
}

/** Terminale aperto di un workspace, come mostrato nella sua card. */
export interface SidebarTerminal {
  key: string;
  label: string;
  /** Nome del bot della sessione; assente = funzione spenta. */
  bot?: string;
  /** Immagine del bot (se il suo nome ne ha una). */
  botImage?: BotImage;
  status: TabStatus;
  pinned: boolean;
  /** È il pannello attivo del gruppo mostrato. */
  active: boolean;
}

export interface TitleBarTabs {
  /** Vista globale: presente solo con almeno un tab fissato. */
  globalTab: TitleTab | null;
  groups: TitleTab[];
}

/**
 * Etichetta di un tab: titolo del task (worker) > titolo OSC > nome
 * dell'avvio > nome del workspace. Il worker mostra il titolo del task perché
 * il titolo OSC dell'agente cambia di continuo.
 */
export function tabLabel(
  tab: TabEntry,
  rt: TabRuntime | undefined,
  ws: Workspace | undefined,
  builtins: BuiltinLaunch[],
): string {
  if (rt?.automation?.title) return rt.automation.title;
  const launchName =
    tab.launchId && ws ? allLaunches(ws, builtins).find((l) => l.id === tab.launchId)?.name : undefined;
  return rt?.title ?? launchName ?? ws?.name ?? "";
}

/** Indicatore di un gruppo dai suoi stati: ❗ > ✓ > in corso > nessuno. */
function indicatorStatus(statuses: TabStatus[]): TabStatus | null {
  const indicator = groupIndicator(statuses);
  return indicator === "none" ? null : GROUP_STATUS[indicator];
}

/** Tab della barra del titolo: vista globale (se ha tab fissati) + un tab per gruppo. */
export function titleBarTabs(
  layout: LayoutState,
  tabs: TabsState,
  runtime: RuntimeState,
  workspaces: readonly Workspace[],
): TitleBarTabs {
  const statusByKey = new Map(tabs.map((t) => [t.key, statusOf(runtime[t.key])]));
  const statusesOf = (keys: string[]): TabStatus[] =>
    keys.flatMap((k) => {
      const status = statusByKey.get(k);
      return status ? [status] : [];
    });
  const globalOrder = layout.global.order;
  return {
    globalTab:
      globalOrder.length > 0
        ? {
            id: GLOBAL_ID,
            name: GLOBAL_TAB_NAME,
            count: globalOrder.length,
            status: indicatorStatus(statusesOf(globalOrder)),
          }
        : null,
    groups: layout.groups.map((g) => ({
      id: g.workspaceId,
      name: workspaces.find((w) => w.id === g.workspaceId)?.name ?? g.workspaceId,
      count: g.order.length,
      status: indicatorStatus(statusesOf(g.order)),
    })),
  };
}

function sameTitleTab(a: TitleTab, b: TitleTab): boolean {
  return a.id === b.id && a.name === b.name && a.count === b.count && sameStatus(a.status, b.status);
}

/** Stessi tab della barra del titolo? (gli oggetti `TabStatus` sono nuovi a ogni derivazione) */
export function sameTitleTabs(a: TitleBarTabs, b: TitleBarTabs): boolean {
  const sameGlobal =
    a.globalTab === null || b.globalTab === null ? a.globalTab === b.globalTab : sameTitleTab(a.globalTab, b.globalTab);
  return sameGlobal && a.groups.length === b.groups.length && a.groups.every((g, i) => sameTitleTab(g, b.groups[i]));
}

/**
 * Terminali di un workspace per la sidebar, nell'ordine della griglia. Il bot
 * (nome e immagine) compare solo con la funzione attiva.
 */
export function sidebarTerminals(
  workspaceId: string,
  layout: LayoutState,
  tabs: TabsState,
  runtime: RuntimeState,
  workspaces: readonly Workspace[],
  builtins: BuiltinLaunch[],
  bots: BotSettings,
  botImageOf: (name: string) => BotImage | undefined,
): SidebarTerminal[] {
  const group = layout.groups.find((g) => g.workspaceId === workspaceId);
  if (!group) return [];
  const tabByKey = new Map(tabs.map((t) => [t.key, t]));
  return group.order.flatMap((key) => {
    const tab = tabByKey.get(key);
    if (!tab) return [];
    const rt = runtime[key];
    const ws = workspaces.find((w) => w.id === tab.workspaceId);
    return [
      {
        key,
        label: tabLabel(tab, rt, ws, builtins),
        bot: shownBot(bots, tab.bot),
        botImage: bots.enabled ? botImageOf(tab.bot) : undefined,
        status: statusOf(rt),
        pinned: layout.global.order.includes(key),
        active: group.activeTabId === key,
      },
    ];
  });
}

/** Stessi terminali della sidebar? (per non ridisegnare la card se non cambia nulla di visibile) */
export function sameSidebarTerminals(a: SidebarTerminal[], b: SidebarTerminal[]): boolean {
  return (
    a.length === b.length &&
    a.every((t, i) => {
      const u = b[i];
      return (
        t.key === u.key &&
        t.label === u.label &&
        t.bot === u.bot &&
        t.botImage === u.botImage &&
        t.pinned === u.pinned &&
        t.active === u.active &&
        sameStatus(t.status, u.status)
      );
    })
  );
}

/** Quanti terminali aspettano una risposta (❗). */
export function attentionCount(tabs: TabsState, runtime: RuntimeState): number {
  return countNeedingInput(tabs.map((t) => statusOf(runtime[t.key])));
}
