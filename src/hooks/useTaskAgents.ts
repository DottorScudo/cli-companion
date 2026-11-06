import { useMemo } from "react";
import { configStore } from "../stores/config";
import { taskAgents } from "../lib/taskAgent";
import type { TaskAgents } from "../lib/taskAgent";
import type { Workspace } from "../lib/tauri";
import { useStore } from "./useStore";

/** Gli agenti dei task di un workspace: scelte nei form, badge, motivo di un agente non pronto. */
export function useTaskAgents(workspace: Workspace): TaskAgents {
  const integrations = useStore(configStore, (s) => s.integrations);
  const bots = useStore(configStore, (s) => s.botSettings.agents);
  return useMemo(() => taskAgents(integrations, workspace, bots), [integrations, workspace, bots]);
}
