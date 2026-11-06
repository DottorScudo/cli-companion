import { memo, useMemo } from "react";
import { useStore } from "../../hooks/useStore";
import { useOverlayShown } from "../../hooks/useView";
import {
  activatePane,
  paneDragEnd,
  paneDragLeave,
  paneDragOver,
  paneDragStart,
  paneDrop,
  toggleMaximizePane,
  togglePinPane,
} from "../../app/actions/panes";
import { closeTab, handleExit, handleInput, handleStatus, handleTitle } from "../../app/actions/tabs";
import { registerFlush } from "../../app/lifecycle";
import { resolveInputOverride, tabAgentId, tabStatus } from "../../lib/agents";
import { automationTooltip, isIntakeLaunch } from "../../lib/automation";
import { GLOBAL_ID } from "../../lib/layout";
import { usageText, usageTooltip } from "../../lib/usage";
import { botImageResolver, configStore, shownBot } from "../../stores/config";
import { layoutStore, samePaneLayout, shownPaneLayouts } from "../../stores/layout";
import { paneDragStore } from "../../stores/paneDrag";
import { tabLabel } from "../../stores/selectors";
import { runtimeStore } from "../../stores/tabRuntime";
import { findTab, tabsStore } from "../../stores/tabs";
import { usageStore } from "../../stores/usage";
import { workspaceById, workspacesStore } from "../../stores/workspaces";
import TerminalView from "../TerminalView";

interface Props {
  tabId: string;
}

/**
 * Un pannello della griglia: legge dagli archivi solo ciò che mostra (la sua
 * voce dei tab, il suo stato volatile, il suo layout, il suo workspace, il suo
 * utilizzo) e passa a `TerminalView` valori e callback stabili. Un titolo, uno
 * stato o un layout cambiati ridisegnano solo il pannello interessato.
 */
function TerminalPane({ tabId }: Props) {
  const tab = useStore(tabsStore, (s) => findTab(s, tabId));
  const rt = useStore(runtimeStore, (s) => s[tabId]);
  const pane = useStore(layoutStore, (s) => shownPaneLayouts(s)[tabId], samePaneLayout);
  const workspace = useStore(workspacesStore, (s) => (tab ? workspaceById(s, tab.workspaceId) : undefined));
  const dropTarget = useStore(paneDragStore, (s) => s.over === tabId);
  const overlayShown = useOverlayShown();
  const sessionUsage = useStore(usageStore, (s) => s.sessions[tabId]);
  const showUsage = useStore(configStore, (s) => s.showUsage);
  const bots = useStore(configStore, (s) => s.botSettings);
  const builtins = useStore(configStore, (s) => s.builtinLaunches);
  const profiles = useStore(configStore, (s) => s.agentProfiles);

  const agent = rt?.agent;
  const busy = rt?.busy === true;
  const automation = rt?.automation;
  const ended = agent?.ended === true;

  const status = useMemo(() => tabStatus(agent, busy), [agent, busy]);
  const automationInfo = useMemo(
    () => (automation ? { status: automation.status, tooltip: automationTooltip(automation) } : undefined),
    [automation],
  );
  const usage = useMemo(
    () =>
      showUsage && sessionUsage && !ended
        ? { text: usageText(sessionUsage), title: usageTooltip(sessionUsage) }
        : undefined,
    [showUsage, sessionUsage, ended],
  );

  // Tab chiuso nel frattempo: la griglia lo toglie al prossimo render.
  if (!tab) return null;

  const label = tabLabel(tab, rt, workspace, builtins);
  const imagePasteSeq = workspace
    ? resolveInputOverride(
        profiles,
        tabAgentId({ agent, startCommand: tab.startCommand }, profiles),
        workspace.kind,
        "image-paste",
      )
    : null;

  return (
    <TerminalView
      tabId={tabId}
      workspaceId={tab.workspaceId}
      initialCommand={tab.startCommand}
      imagePasteSeq={imagePasteSeq}
      restore={tab.restored}
      rect={pane?.rect ?? null}
      active={pane?.visible ?? false}
      focused={(pane?.focused ?? false) && !overlayShown}
      label={label}
      bot={shownBot(bots, tab.bot)}
      botImage={bots.enabled ? botImageResolver(bots)(tab.bot) : undefined}
      labelPrefix={pane?.groupWorkspaceId === GLOBAL_ID ? workspace?.name : undefined}
      status={status}
      cwd={tab.cwd}
      intake={isIntakeLaunch(tab.launchId, bots.agents)}
      usage={usage}
      automation={automationInfo}
      maximized={pane?.maximized ?? false}
      pinned={pane?.pinned ?? false}
      dropTarget={dropTarget}
      onActivate={activatePane}
      onClose={closeTab}
      onToggleMaximize={toggleMaximizePane}
      onTogglePin={togglePinPane}
      onPaneDragStart={paneDragStart}
      onPaneDragOver={paneDragOver}
      onPaneDragLeave={paneDragLeave}
      onPaneDrop={paneDrop}
      onPaneDragEnd={paneDragEnd}
      registerFlush={registerFlush}
      onExit={handleExit}
      onTitle={handleTitle}
      onStatus={handleStatus}
      onInput={handleInput}
    />
  );
}

export default memo(TerminalPane);
