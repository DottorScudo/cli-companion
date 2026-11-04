import { memo } from "react";
import HubView from "../hub/HubView";

/**
 * Il mondo Agents HUB sta sopra pannello Task e terminali, come l'Ufficio
 * (z-30): i TerminalView restano montati al loro posto, di pari dimensioni (la
 * colonna dei canali è a sinistra, fuori dall'area di lavoro). Montato solo nel suo mondo.
 */
function HubOverlay() {
  return (
    <div className="absolute inset-[3px] z-30 overflow-hidden rounded-[10px] border bg-pane-area">
      <HubView />
    </div>
  );
}

export default memo(HubOverlay);
