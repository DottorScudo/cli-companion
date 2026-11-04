import { Fragment } from "react";
import { MessagesSquare } from "lucide-react";
import { useHubChannels } from "../hooks/useHubChannels";
import { useStore } from "../hooks/useStore";
import { hubTitlePath } from "../lib/hub";
import { hubStore } from "../stores/hub";

const Separator = () => (
  <span className="text-muted-foreground" aria-hidden="true">
    /
  </span>
);

/**
 * Percorso nella barra del titolo del mondo Agents HUB: Agents HUB / # canale /
 * titolo del thread aperto (se già letto). Non intercetta il mouse: la barra
 * resta una zona di trascinamento.
 */
export default function HubTitlePath() {
  const { channels, current } = useHubChannels();
  const threadTitle = useStore(hubStore, (s) => s.thread?.title ?? null);
  const path = hubTitlePath(channels, current, threadTitle);
  const parts = [`# ${path.channelName}`, ...(path.threadTitle === null ? [] : [path.threadTitle])];

  return (
    <nav
      aria-label="Posizione"
      className="pointer-events-none flex min-w-0 items-center gap-2 text-[13px] text-text-secondary"
    >
      <span className="flex flex-shrink-0 items-center gap-1.5 font-semibold text-primary">
        <MessagesSquare className="!size-3.5" aria-hidden="true" />
        Agents HUB
      </span>
      {parts.map((part, i) => (
        <Fragment key={i}>
          <Separator />
          <span className={i === parts.length - 1 ? "min-w-0 truncate text-foreground" : "flex-shrink-0"}>{part}</span>
        </Fragment>
      ))}
    </nav>
  );
}
