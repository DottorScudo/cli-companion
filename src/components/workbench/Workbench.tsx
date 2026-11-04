import { useEffect } from "react";
import { useStore } from "../../hooks/useStore";
import { useOfficeShown, useOverlayShown } from "../../hooks/useView";
import { tabsStore } from "../../stores/tabs";
import { useStableDesks } from "../office/useStableDesks";
import ManagerOverlay from "./ManagerOverlay";
import OfficeOverlay from "./OfficeOverlay";
import TaskPanelArea from "./TaskPanelArea";
import TerminalGrid from "./TerminalGrid";

/** Manager e Ufficio coprono i terminali: nessuno deve tenere il focus della tastiera. */
function useBlurTerminalsUnder(overlayShown: boolean): void {
  useEffect(() => {
    if (!overlayShown) return;
    const el = document.activeElement;
    if (el instanceof HTMLElement && el.closest(".xterm")) el.blur();
  }, [overlayShown]);
}

/**
 * Area di lavoro: pannello Task a sinistra, area dei pannelli e, sopra, Tasks
 * Manager e Ufficio. Vive quanto l'app: tiene le scrivanie dei bot, stabili per
 * tutta la vita dei tab anche ad Ufficio chiuso.
 */
export default function Workbench() {
  const tabs = useStore(tabsStore);
  const desks = useStableDesks(tabs);
  const officeShown = useOfficeShown();
  useBlurTerminalsUnder(useOverlayShown());

  return (
    <div className="relative flex min-w-0 flex-1">
      <TaskPanelArea />
      <TerminalGrid />
      <ManagerOverlay />
      {officeShown && <OfficeOverlay desks={desks} />}
    </div>
  );
}
