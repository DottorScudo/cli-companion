import { memo } from "react";
import HubView from "../hub/HubView";

/**
 * L'Agents HUB sta sopra tutto, come l'Ufficio (z-30): i TerminalView restano
 * montati al loro posto. Montato solo mentre è aperto.
 */
function HubOverlay() {
  return (
    <div className="absolute inset-[3px] z-30 overflow-hidden rounded-[10px] border bg-pane-area">
      <HubView />
    </div>
  );
}

export default memo(HubOverlay);
