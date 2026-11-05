import { setHubViewMode } from "../../app/actions/hubMessenger";
import { useStore } from "../../hooks/useStore";
import type { HubViewMode } from "../../lib/hubViewMode";
import { hubStore } from "../../stores/hub";
import HubSegmented from "./HubSegmented";

const MODES: { id: HubViewMode; label: string }[] = [
  { id: "channels", label: "Canali" },
  { id: "messenger", label: "Chat" },
];

/** Interruttore Canali | Chat in cima alla colonna sinistra: come si mostrano i thread (preferenza globale). */
export default function HubViewSwitch() {
  const mode = useStore(hubStore, (s) => s.viewMode);
  return (
    <HubSegmented label="Vista" options={MODES} value={mode} onChange={setHubViewMode} className="px-3 pb-2 pt-3" />
  );
}
