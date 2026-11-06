import { PauseCircle } from "lucide-react";
import { Button } from "../ui/button";
import { openIntegrationSettings } from "../../app/actions/settings";
import { unavailableDetail, unavailableText } from "../../lib/taskAgent";
import type { TaskAgents } from "../../lib/taskAgent";
import type { Task } from "../../lib/board";

interface Props {
  task: Pick<Task, "agent_unavailable" | "can_edit" | "bot">;
  agents: TaskAgents;
  /** Apre la modifica del task, dove si sceglie un altro Bot (o un altro agente, se il task non ha un Bot). */
  onChangeAgent: () => void;
}

const btn = "h-[30px] bg-transparent px-3 text-[13px]";

/** Perché un task in coda non parte (il suo agente non è pronto) e cosa fare: cambiare agente o sistemare le integrazioni. */
export default function DetailAgentUnavailable({ task, agents, onChangeAgent }: Props) {
  const unavailable = task.agent_unavailable;
  const text = unavailableText(agents, unavailable);
  if (!unavailable || !text) return null;
  return (
    <section
      aria-label="Agente non disponibile"
      className="flex flex-col gap-2 rounded-md border border-status-running/40 bg-surface px-3 py-2.5"
    >
      <p className="flex items-center gap-1.5 text-[13px] font-medium text-status-running">
        <PauseCircle className="h-3.5 w-3.5 flex-shrink-0" aria-hidden="true" />
        {text}
      </p>
      <p className="break-words text-xs text-text-secondary">{unavailableDetail(agents, unavailable)}</p>
      <div className="flex flex-wrap gap-1.5">
        {task.can_edit && (
          <Button type="button" variant="outline" onClick={onChangeAgent} className={btn}>
            {task.bot ? "Cambia Bot" : "Cambia agente"}
          </Button>
        )}
        <Button type="button" variant="outline" onClick={openIntegrationSettings} className={btn}>
          Apri Integrazioni
        </Button>
      </div>
    </section>
  );
}
