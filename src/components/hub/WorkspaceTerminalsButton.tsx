import { SquareTerminal } from "lucide-react";
import { showWorkspaceTerminals } from "../../app/actions/view";
import { GENERAL_CHANNEL } from "../../lib/hub";
import { Button } from "../ui/button";

/** «Terminali» del workspace di un canale; niente per il Generale, che non ne ha. */
export default function WorkspaceTerminalsButton({ channel, channelName }: { channel: string; channelName: string }) {
  if (channel === GENERAL_CHANNEL) return null;
  return (
    <Button
      type="button"
      size="sm"
      variant="outline"
      className="h-7 gap-1 px-2"
      onClick={() => showWorkspaceTerminals(channel)}
      title={`Vai ai terminali di ${channelName}`}
    >
      <SquareTerminal className="!size-3.5" aria-hidden="true" /> Terminali
    </Button>
  );
}
