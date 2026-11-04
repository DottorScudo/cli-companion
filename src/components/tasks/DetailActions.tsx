import { useState } from "react";
import {
  ArrowUpToLine,
  CheckCheck,
  ExternalLink,
  Pencil,
  RotateCw,
  SquareTerminal,
  Trash2,
  OctagonX,
} from "lucide-react";
import { Button } from "../ui/button";
import { ACTION_LABELS, prUrl, queuedTasks } from "../../lib/board";
import type { BoardView, Task, TaskAction } from "../../lib/board";
import { canDo, deleteControl } from "../../lib/taskActions";

interface Props {
  task: Task;
  board: BoardView;
  busy: boolean;
  /** Il tab del worker si può aprire. */
  canOpenTerminal: boolean;
  onEdit: () => void;
  onAction: (action: TaskAction) => void;
  onOpenTerminal: () => void;
  onOpenPr: (url: string) => void;
}

type Confirm = "delete" | "delete_cascade" | "stop" | "mark_done";

const IRREVERSIBLE = "L'operazione non si può annullare.";

const CONFIRM_TEXT: Record<Exclude<Confirm, "delete_cascade">, string> = {
  delete: `Eliminare questo task? ${IRREVERSIBLE}`,
  stop: "Fermare il worker? Il task passa a Fallito e il terminale viene chiuso.",
  mark_done: "Segnare il task come fatto? Il lavoro non viene verificato né integrato dall'app.",
};

function cascadeConfirmText(titles: string[]): string {
  const subject = titles.length === 1 ? "il task che ne dipende" : `i ${titles.length} task che ne dipendono`;
  return `Eliminare questo task e ${subject}? ${titles.join(", ")} - ${IRREVERSIBLE}`;
}

function dependentTitles(ids: string[], board: BoardView): string[] {
  const titles = new Map(board.tasks.map((t) => [t.id, t.title]));
  return ids.map((id) => titles.get(id) ?? id);
}

/** Stati in cui ha senso aprire il tab del worker (in corso, o fallito con il tab ancora aperto). */
const TERMINAL_STATUSES: Task["status"][] = ["running", "failed"];

const btn = "h-[30px] gap-1.5 px-3 text-[13px]";
const ghost = `${btn} bg-transparent`;

/** Azioni che il backend consente sul task; Elimina, Ferma e Segna come fatto chiedono conferma in linea. */
export default function DetailActions({
  task,
  board,
  busy,
  canOpenTerminal,
  onEdit,
  onAction,
  onOpenTerminal,
  onOpenPr,
}: Props) {
  const [confirm, setConfirm] = useState<Confirm | null>(null);
  const { cascade } = task;

  if (confirm) {
    const confirmText =
      confirm === "delete_cascade"
        ? cascadeConfirmText(dependentTitles(cascade.dependents, board))
        : CONFIRM_TEXT[confirm];
    return (
      <div
        role="alertdialog"
        aria-label="Conferma"
        className="flex flex-col gap-2 rounded-md border border-destructive/40 bg-destructive/5 p-2.5"
      >
        <p className="text-xs">{confirmText}</p>
        <div className="flex gap-2">
          <Button
            type="button"
            variant={confirm === "mark_done" ? "default" : "destructive"}
            disabled={busy}
            autoFocus
            onClick={() => {
              onAction(confirm);
              setConfirm(null);
            }}
            className={btn}
          >
            {ACTION_LABELS[confirm]}
          </Button>
          <Button type="button" variant="outline" onClick={() => setConfirm(null)} className={ghost}>
            Annulla
          </Button>
        </div>
      </div>
    );
  }

  const pr = prUrl(task);
  const remove = deleteControl(task);
  const showTerminal = canOpenTerminal && TERMINAL_STATUSES.includes(task.status);
  // Con "Riprova" è lui il pulsante principale.
  const terminalVariant = canDo(task, "retry") ? "outline" : "default";

  return (
    <div className="flex flex-wrap gap-1.5">
      {canDo(task, "retry") && (
        <Button type="button" disabled={busy} onClick={() => onAction("retry")} className={btn}>
          <RotateCw /> {ACTION_LABELS.retry}
        </Button>
      )}
      {canDo(task, "mark_done") && (
        <Button
          type="button"
          variant="outline"
          disabled={busy}
          onClick={() => setConfirm("mark_done")}
          className={ghost}
        >
          <CheckCheck /> {ACTION_LABELS.mark_done}
        </Button>
      )}
      {task.can_edit && (
        <Button type="button" variant="outline" disabled={busy} onClick={onEdit} className={ghost}>
          <Pencil /> Modifica
        </Button>
      )}
      {canDo(task, "to_top") && <ToTopButton task={task} board={board} busy={busy} onAction={onAction} />}
      {showTerminal && (
        <Button
          type="button"
          variant={terminalVariant}
          onClick={onOpenTerminal}
          className={terminalVariant === "outline" ? ghost : btn}
        >
          <SquareTerminal /> Apri terminale
        </Button>
      )}
      {canDo(task, "stop") && (
        <Button type="button" variant="outline" disabled={busy} onClick={() => setConfirm("stop")} className={ghost}>
          <OctagonX /> {ACTION_LABELS.stop}
        </Button>
      )}
      {task.status === "done" && pr && (
        <Button type="button" onClick={() => onOpenPr(pr)} className={btn}>
          <ExternalLink /> Apri PR
        </Button>
      )}
      {remove && (
        <Button
          type="button"
          variant="outline"
          disabled={busy || remove.kind === "blocked"}
          title={
            remove.kind === "blocked"
              ? `Prima ferma i task in esecuzione che ne dipendono: ${remove.running.join(", ")}`
              : undefined
          }
          onClick={() => remove.kind !== "blocked" && setConfirm(remove.kind)}
          className={`${ghost} text-destructive hover:text-destructive`}
        >
          <Trash2 /> {ACTION_LABELS.delete}
        </Button>
      )}
    </div>
  );
}

/** "In cima alla coda": disattivato se il task è già il prossimo. */
function ToTopButton({ task, board, busy, onAction }: Pick<Props, "task" | "board" | "busy" | "onAction">) {
  const isFirst = queuedTasks(board)[0]?.id === task.id;
  return (
    <Button
      type="button"
      variant="outline"
      disabled={busy || isFirst}
      title={isFirst ? "È già il prossimo in coda" : undefined}
      onClick={() => onAction("to_top")}
      className={ghost}
    >
      <ArrowUpToLine /> {ACTION_LABELS.to_top}
    </Button>
  );
}
