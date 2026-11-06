import { PanelRight } from "lucide-react";
import { useStore } from "../../hooks/useStore";
import { hubStore } from "../../stores/hub";
import { Button } from "../ui/button";

const LABEL = "Mostra/Nascondi task e terminali";

/** Pulsante dell'intestazione che mostra o nasconde il pannello task e terminali del thread. */
export default function ThreadPanelToggle() {
  const shown = useStore(hubStore, (s) => s.threadPanelShown);
  return (
    <Button
      type="button"
      size="icon"
      variant={shown ? "secondary" : "outline"}
      className="size-7 flex-shrink-0"
      aria-label={LABEL}
      aria-pressed={shown}
      title={LABEL}
      onClick={() => hubStore.threadPanelShownSet(!shown)}
    >
      <PanelRight className="!size-3.5" />
    </Button>
  );
}
