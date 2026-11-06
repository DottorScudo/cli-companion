import { focusTerminal, openManager } from "../../app/actions/view";
import { displayStatus, statusDotClass, statusLabel, statusTextClass } from "../../lib/board";
import { isPlan } from "../../lib/plan";
import type { HubThread } from "../../lib/tauri";
import BotAvatar from "../BotAvatar";
import StatusIcon from "../StatusIcon";
import { PlanBadge } from "../tasks/PlanBadge";
import { useActiveThreadPanelTasks, useThreadPanelTerminals } from "./useThreadPanel";

function Section({
  title,
  count,
  empty,
  children,
}: {
  title: string;
  count: number;
  empty: string;
  children: React.ReactNode;
}) {
  return (
    <section aria-label={title} className="border-b">
      <h3 className="flex items-center gap-2 px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
        {title}
        <span className="rounded border px-1.5 py-px text-[10px] font-normal normal-case tracking-normal">{count}</span>
      </h3>
      {count === 0 ? <p className="px-3 pb-3 text-xs text-muted-foreground">{empty}</p> : <ul>{children}</ul>}
    </section>
  );
}

const ROW_CLASS =
  "flex w-full items-center gap-2 px-3 py-1.5 text-left text-[13px] transition-colors hover:bg-accent/50";

function TaskRows({ thread }: { thread: HubThread }) {
  const tasks = useActiveThreadPanelTasks(thread);
  return (
    <Section title="Task" count={tasks.length} empty="Nessun task in corso o in coda">
      {tasks.map(({ task, workspace }) => {
        const status = displayStatus(task);
        return (
          <li key={task.id}>
            <button
              type="button"
              onClick={() => openManager(workspace, { taskId: task.id })}
              title={`${task.title} · ${statusLabel(status)}`}
              className={ROW_CLASS}
            >
              <span className={`h-2 w-2 flex-shrink-0 rounded-full ${statusDotClass(status)}`} aria-hidden="true" />
              <span className="min-w-0 flex-1 truncate">{task.title}</span>
              {isPlan(task) && <PlanBadge className="text-[10px]" />}
              <span className={`flex-shrink-0 text-[11px] ${statusTextClass(status)}`}>{statusLabel(status)}</span>
            </button>
          </li>
        );
      })}
    </Section>
  );
}

function TerminalRows({ thread }: { thread: HubThread }) {
  const terminals = useThreadPanelTerminals(thread);
  return (
    <Section title="Terminali" count={terminals.length} empty="Nessun terminale aperto">
      {terminals.map((t) => {
        const name = t.bot ? `${t.bot} · ${t.label}` : t.label;
        return (
          <li key={t.key}>
            <button type="button" onClick={() => focusTerminal(t.key)} title={name} className={ROW_CLASS}>
              <span className="flex w-3.5 flex-shrink-0 justify-center">
                <StatusIcon status={t.status} />
              </span>
              {t.bot && <BotAvatar name={t.bot} size={16} image={t.botImage} />}
              <span className="min-w-0 flex-1 truncate font-mono text-xs">{name}</span>
            </button>
          </li>
        );
      })}
    </Section>
  );
}

/** Colonna a destra della conversazione: anteprima dei task del thread e dei terminali ancora aperti dei loro worker. */
export default function HubThreadSidePanel({ thread }: { thread: HubThread }) {
  return (
    <aside
      aria-label="Task e terminali del thread"
      className="flex w-[270px] flex-shrink-0 flex-col overflow-y-auto border-l"
    >
      <TaskRows thread={thread} />
      <TerminalRows thread={thread} />
    </aside>
  );
}
