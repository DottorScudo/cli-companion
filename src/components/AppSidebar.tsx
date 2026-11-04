import { Loader2 } from "lucide-react";
import Sidebar from "./sidebar/Sidebar";
import SidebarFooter from "./sidebar/SidebarFooter";
import { useStore } from "../hooks/useStore";
import { uiStore } from "../stores/ui";
import { workspacesStore } from "../stores/workspaces";

/** Colonna di sinistra: elenco dei workspace (o caricamento / errore) e piede con le Impostazioni. */
export default function AppSidebar() {
  const status = useStore(workspacesStore, (s) => s.status);
  const error = useStore(workspacesStore, (s) => s.error);
  const sessionReady = useStore(uiStore, (s) => s.sessionReady);

  return (
    <aside className="flex w-[260px] flex-shrink-0 flex-col overflow-hidden border-r bg-base">
      {status === "error" ? (
        <div className="p-4">
          <p className="text-[13px] text-destructive">{error}</p>
        </div>
      ) : status === "loading" || !sessionReady ? (
        <div className="flex items-center gap-2 p-4 text-sm text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" />
          Caricamento…
        </div>
      ) : (
        <Sidebar />
      )}
      <SidebarFooter />
    </aside>
  );
}
