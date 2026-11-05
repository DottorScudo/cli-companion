import { openTaskThread } from "../../app/actions/hub";
import { useStore } from "../../hooks/useStore";
import { formatRelative, statusLabel } from "../../lib/board";
import type { Task } from "../../lib/board";
import { isPlan } from "../../lib/plan";
import { hubStore } from "../../stores/hub";
import { Button } from "../ui/button";
import DetailSection from "./DetailSection";

interface Props {
  workspaceId: string;
  task: Task;
  now: number;
}

/**
 * Domanda in sospeso del worker (o del pianificatore): testo, risposte suggerite e
 * da quanto tempo aspetta. La risposta si dà dall'Agents HUB, che il pulsante apre
 * sul thread della domanda.
 */
export default function DetailQuestion({ workspaceId, task, now }: Props) {
  const hubEnabled = useStore(hubStore, (s) => s.enabled);
  const question = task.question;
  if (!question) return null;
  return (
    <DetailSection title={isPlan(task) ? "Domanda del pianificatore" : "Domanda del worker"}>
      <div className="flex flex-col gap-1.5 rounded-md border border-primary/40 bg-surface px-3 py-2 text-[13px]">
        <p className="text-[11px] font-medium text-primary">{statusLabel("question")}</p>
        <p className="whitespace-pre-line break-words">{question.text}</p>
        {question.options && question.options.length > 0 && (
          <ul className="list-disc pl-4 text-xs text-text-secondary">
            {question.options.map((option) => (
              <li key={option} className="break-words">
                {option}
              </li>
            ))}
          </ul>
        )}
        <p className="text-[11px] text-muted-foreground">Chiesta {formatRelative(question.asked_at, now)}</p>
        {hubEnabled && (
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => void openTaskThread(workspaceId, task.id)}
            className="mt-1 h-7 self-start px-2.5 text-xs"
          >
            Rispondi nell'Agents HUB
          </Button>
        )}
      </div>
    </DetailSection>
  );
}
