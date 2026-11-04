import AppDialogs from "./components/AppDialogs";
import AppSidebar from "./components/AppSidebar";
import AppStatusBar from "./components/AppStatusBar";
import AppTitleBar from "./components/AppTitleBar";
import Workbench from "./components/workbench/Workbench";
import { useAppEvents } from "./hooks/useAppEvents";
import { useBackgroundChecks } from "./hooks/useBackgroundChecks";
import { useBoardsSync, useTaskNotify } from "./hooks/useBoards";
import { useFrontendReady } from "./hooks/useFrontendReady";
import { useHubSync } from "./hooks/useHubSync";
import { useInitialData } from "./hooks/useInitialData";
import { useIntegrationsSync } from "./hooks/useIntegrations";
import { useLaunchesSync } from "./hooks/useLaunchesSync";
import { useSessionPersistence } from "./hooks/useSessionPersistence";
import { useShortcuts } from "./hooks/useShortcuts";
import { useStoreWarnings } from "./hooks/useStoreWarnings";
import { useUsagePolling } from "./hooks/useUsage";

/**
 * Composizione dell'app: lo stato vive negli archivi (`src/stores`), le azioni
 * in `src/app`, e ogni componente legge da sé ciò che mostra. Qui si avviano
 * solo i processi di fondo e si dispongono le aree.
 */
export default function App() {
  useInitialData();
  useIntegrationsSync();
  useLaunchesSync();
  useStoreWarnings();
  useBoardsSync();
  useTaskNotify();
  useHubSync();
  useUsagePolling();
  useSessionPersistence();
  useShortcuts();
  useBackgroundChecks();
  // Il backend parte solo a sessione ripristinata e ascoltatori registrati.
  useFrontendReady(useAppEvents());

  return (
    <div className="flex h-screen flex-col">
      <AppTitleBar />
      <div className="flex min-h-0 flex-1">
        <AppSidebar />
        <Workbench />
      </div>
      <AppStatusBar />
      <AppDialogs />
    </div>
  );
}
