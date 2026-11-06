import { INTERACTIVE_ONLY_LABEL, TASK_KINDS, TASK_KIND_LABELS } from "../../../lib/taskBot";
import type { TaskKind } from "../../../bindings/TaskKind";
import { Field } from "../shared";

interface Props {
  kinds: readonly TaskKind[];
  disabled: boolean;
  onChange: (kinds: TaskKind[]) => void;
}

/** I tipi di task automatici che il Bot può eseguire; nessuno = solo terminali interattivi. */
export default function BotKindsField({ kinds, disabled, onChange }: Props) {
  const toggle = (kind: TaskKind, checked: boolean) =>
    onChange(TASK_KINDS.filter((k) => (k === kind ? checked : kinds.includes(k))));
  return (
    <Field
      label="Tipi di task"
      help={
        kinds.length === 0
          ? `${INTERACTIVE_ONLY_LABEL}: il Bot non viene scelto per i task della board.`
          : "I task della board che il Bot può eseguire."
      }
    >
      <div role="group" aria-label="Tipi di task" className="flex flex-wrap gap-x-4 gap-y-1">
        {TASK_KINDS.map((kind) => (
          <label key={kind} className="flex items-center gap-2 text-[13px]">
            <input
              type="checkbox"
              checked={kinds.includes(kind)}
              disabled={disabled}
              onChange={(e) => toggle(kind, e.target.checked)}
              className="h-3.5 w-3.5 accent-primary"
            />
            {TASK_KIND_LABELS[kind]}
          </label>
        ))}
      </div>
    </Field>
  );
}
