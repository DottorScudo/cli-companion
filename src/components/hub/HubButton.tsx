import { MessagesSquare } from "lucide-react";
import { Button } from "../ui/button";
import { cn } from "../../lib/utils";

interface Props {
  /** La vista Agents HUB è aperta. */
  shown: boolean;
  onToggle: () => void;
}

/** Pulsante del piede della Sidebar che apre/chiude l'Agents HUB. */
export default function HubButton({ shown, onToggle }: Props) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon"
      onClick={onToggle}
      aria-label="Agents HUB"
      aria-pressed={shown}
      title="Agents HUB"
      className={cn(
        "h-8 w-8 flex-shrink-0 text-text-secondary hover:text-foreground",
        shown && "bg-accent text-foreground",
      )}
    >
      <MessagesSquare className="!size-[15px]" />
    </Button>
  );
}
