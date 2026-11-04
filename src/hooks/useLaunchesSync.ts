import { useEffect } from "react";
import { refreshLaunches } from "../app/actions/launches";
import { report } from "../lib/errors";
import { workspaceAgents } from "../lib/launch";
import { configStore, defaultIntegration, enabledIntegrations } from "../stores/config";
import { workspacesStore } from "../stores/workspaces";
import { useStore } from "./useStore";

/**
 * Avvii integrati nello store `configStore`: riletti quando cambiano le
 * integrazioni abilitate o la predefinita e gli agenti propri dei workspace (da
 * cui dipendono gli agenti dei preset). Senza di loro l'app non ha avvii
 * integrati: l'utente va avvisato.
 */
export function useLaunchesSync(): void {
  const enabled = useStore(configStore, (s) =>
    enabledIntegrations(s)
      .map((i) => i.id)
      .join("\n"),
  );
  const defaultId = useStore(configStore, (s) => defaultIntegration(s)?.id ?? "");
  const agents = useStore(workspacesStore, (s) => workspaceAgents(s.list).join("\n"));

  useEffect(() => {
    refreshLaunches().catch((e) => report("caricamento degli avvii integrati", e, { notify: true }));
  }, [enabled, defaultId, agents]);
}
