import { openHubSettings } from "../../app/actions/settings";
import { useStore } from "../../hooks/useStore";
import { activityLabel, secretaryActivity } from "../../lib/hubSecretaries";
import type { HubSecretary } from "../../lib/tauri";
import { cn } from "../../lib/utils";
import { hubStore } from "../../stores/hub";
import BotAvatar from "../BotAvatar";
import { useManagerNow } from "../tasks/useManagerNow";

const AVATAR_SIZE = 20;

function SecretaryItem({ secretary, now }: { secretary: HubSecretary; now: number }) {
  const status = useStore(hubStore, (s) => s.secretaryStatus);
  const activity = secretaryActivity(secretary, status);
  const dimmed = activity.kind === "unconfigured";
  return (
    <li className="flex items-center gap-2 rounded-md px-2.5 py-1.5">
      <BotAvatar
        name={secretary.name}
        size={AVATAR_SIZE}
        image={secretary.image ?? undefined}
        className={cn(activity.kind !== "awake" && "opacity-55")}
      />
      <div className="flex min-w-0 flex-1 flex-col">
        <span className={cn("truncate font-mono text-[13px]", dimmed && "text-muted-foreground")}>
          @{secretary.name}
        </span>
        <span className="truncate text-[11px] text-muted-foreground">{activityLabel(activity, now)}</span>
      </div>
      {activity.kind === "awake" && (
        <span
          aria-hidden="true"
          className={cn("size-2 flex-shrink-0 rounded-full bg-status-finished", activity.working && "animate-pulse")}
        />
      )}
    </li>
  );
}

/** Sezione «SEGRETARI» della colonna di sinistra: avatar, nome e stato (sveglio, dorme, non configurato). */
export default function HubSecretaryList() {
  const secretaries = useStore(hubStore, (s) => s.settings?.secretaries);
  const now = useManagerNow();
  return (
    <section aria-labelledby="hub-secretaries-title" className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <h2
        id="hub-secretaries-title"
        className="pb-2 pl-4 pr-3 pt-4 text-[11px] font-semibold uppercase tracking-[0.08em] text-faint"
      >
        Segretari
      </h2>
      {secretaries === undefined || secretaries.length === 0 ? (
        <p className="px-4 text-[11px] text-muted-foreground">
          Nessun segretario.{" "}
          <button type="button" onClick={openHubSettings} className="text-primary underline-offset-2 hover:underline">
            Creane uno nelle impostazioni
          </button>
          .
        </p>
      ) : (
        <ul className="flex min-h-0 flex-col gap-0.5 overflow-y-auto px-2">
          {secretaries.map((s) => (
            <SecretaryItem key={s.name} secretary={s} now={now} />
          ))}
        </ul>
      )}
    </section>
  );
}
