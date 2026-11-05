import { formatRelative } from "../../lib/board";
import type { Task } from "../../lib/board";
import DetailSection from "./DetailSection";

/** Domanda in sospeso del worker: testo, risposte suggerite e da quanto tempo aspetta. La risposta si dà dall'Agents HUB. */
export default function DetailQuestion({ task, now }: { task: Task; now: number }) {
  const question = task.question;
  if (!question) return null;
  return (
    <DetailSection title="Domanda del worker">
      <div className="flex flex-col gap-1.5 rounded-md border border-status-running/40 bg-surface px-3 py-2 text-[13px]">
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
      </div>
    </DetailSection>
  );
}
