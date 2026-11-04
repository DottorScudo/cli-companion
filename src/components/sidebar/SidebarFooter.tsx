import { SlidersHorizontal } from "lucide-react";
import { Button } from "../ui/button";
import HubButton from "../hub/HubButton";
import OfficeButton from "../office/OfficeButton";
import { useStore } from "../../hooks/useStore";
import { useHubShown, useOfficeShown } from "../../hooks/useView";
import { openSettings } from "../../app/actions/settings";
import { toggleHub, toggleOffice } from "../../app/actions/view";
import { configStore } from "../../stores/config";
import { hubStore } from "../../stores/hub";
import { uiStore } from "../../stores/ui";

/** Piede della sidebar: Impostazioni (con il pallino se c'è un aggiornamento), Ufficio, Agents HUB e versione. */
export default function SidebarFooter() {
  const updateAvailable = useStore(uiStore, (s) => s.updateAvailable);
  const botsEnabled = useStore(configStore, (s) => s.botSettings.enabled);
  const appVersion = useStore(configStore, (s) => s.appVersion);
  const hubEnabled = useStore(hubStore, (s) => s.enabled);
  const officeShown = useOfficeShown();
  const hubShown = useHubShown();

  return (
    <div className="flex items-center gap-2 border-t px-3 py-2.5">
      <Button
        variant="ghost"
        onClick={openSettings}
        className="h-auto flex-1 justify-start gap-2 px-2 py-[7px] text-[13px] font-normal text-text-secondary hover:text-foreground"
      >
        <SlidersHorizontal className="!size-[15px]" />
        Impostazioni
        {updateAvailable && (
          <span
            className="h-1.5 w-1.5 rounded-full bg-primary"
            title="Aggiornamento disponibile"
            role="img"
            aria-label="Aggiornamento disponibile"
          />
        )}
      </Button>
      {botsEnabled && <OfficeButton shown={officeShown} onToggle={toggleOffice} />}
      {hubEnabled && <HubButton shown={hubShown} onToggle={toggleHub} />}
      {appVersion && <span className="font-mono text-[11px] text-faint">v{appVersion}</span>}
    </div>
  );
}
