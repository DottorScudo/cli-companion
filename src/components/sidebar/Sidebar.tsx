import { memo, useEffect } from "react";
import { Plus } from "lucide-react";
import { openAddWorkspace } from "../../app/actions/settings";
import { useStore } from "../../hooks/useStore";
import { useViewWorkspaceId } from "../../hooks/useView";
import { uiStore } from "../../stores/ui";
import { visibleWorkspaces, workspacesStore } from "../../stores/workspaces";
import WorkspaceRow from "./WorkspaceRow";
import { sidebarStore } from "./sidebarState";

/**
 * Elenco dei workspace con intestazione e avvisi. Le righe leggono da sole i
 * propri dati: qui cambia solo l'elenco (aggiunta, rimozione, ordine).
 */
function Sidebar() {
  const workspaces = useStore(workspacesStore, visibleWorkspaces);
  const activeWorkspaceId = useViewWorkspaceId();
  // Errore dell'ultima operazione, oppure avviso dell'app (es. avvio rifiutato).
  const error = useStore(sidebarStore, (s) => s.error);
  const notice = useStore(uiStore, (s) => s.notice);

  // La card espansa segue il gruppo attivo (ma si può selezionare anche un
  // workspace senza terminali, che non ha un gruppo).
  useEffect(() => {
    if (activeWorkspaceId) sidebarStore.expand(activeWorkspaceId);
  }, [activeWorkspaceId]);

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex items-center justify-between pb-2 pl-4 pr-3 pt-3.5">
        <span className="text-[11px] font-semibold uppercase tracking-[0.08em] text-faint">Workspace</span>
        <button
          type="button"
          onClick={openAddWorkspace}
          title="Aggiungi workspace"
          aria-label="Aggiungi workspace"
          className="flex h-7 w-7 items-center justify-center rounded-[7px] border border-border-strong text-foreground transition-colors hover:bg-surface-2 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
        >
          <Plus className="h-3.5 w-3.5" />
        </button>
      </div>

      {(error ?? notice) && <p className="px-4 pb-2 text-xs text-destructive">{error ?? notice}</p>}

      <div className="flex flex-1 flex-col gap-0.5 overflow-y-auto px-2">
        {workspaces.length === 0 && (
          <p className="mt-3 px-2 text-[13px] text-muted-foreground">
            Nessun workspace. Aggiungi una cartella per iniziare.
          </p>
        )}
        {workspaces.map((ws) => (
          <WorkspaceRow key={ws.id} ws={ws} />
        ))}
      </div>
    </div>
  );
}

export default memo(Sidebar);
