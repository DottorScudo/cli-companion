import { Bot } from "lucide-react";
import { cn } from "../../lib/utils";

const BADGE_BASE =
  "inline-flex min-w-0 max-w-[10rem] flex-shrink-0 items-center gap-1 rounded bg-surface-2 px-1.5 py-px";

/** Badge con il nome dell'agente che esegue il task; niente se `name` è nullo. */
export default function AgentBadge({ name, className }: { name: string | null; className?: string }) {
  if (!name) return null;
  return (
    <span className={cn(BADGE_BASE, "font-medium text-text-secondary", className)} title={`Agente: ${name}`}>
      <Bot className="h-3 w-3 flex-shrink-0" aria-hidden="true" />
      <span className="truncate">{name}</span>
    </span>
  );
}
