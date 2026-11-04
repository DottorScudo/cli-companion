import type { ReactNode } from "react";
import { terminalCountText } from "../lib/agents";
import { waitingText } from "../lib/hub";
import type { LimitGroup, LimitItem } from "../lib/usage";
import { cn } from "../lib/utils";

interface Props {
  terminalCount: number;
  /** Terminali che aspettano una risposta. */
  attentionCount: number;
  /** Plugin agent installati ("Nome agente · Windows, Ubuntu"); vuoto = nascosto. */
  agentsSummary: string;
  /** Limiti di utilizzo per agente e ambiente; vuoto = nascosti. */
  limits?: LimitGroup[];
  /** Thread dell'Agents HUB che aspettano l'utente; a zero (anche con l'HUB spento) la voce non c'è. */
  hubWaiting?: number;
  onOpenHub?: () => void;
}

const SEVERITY_CLASS: Record<LimitItem["severity"], string> = {
  normal: "",
  muted: "text-faint",
  warning: "text-status-attention",
  critical: "text-destructive",
};

/** Cornice della barra di stato (26px), condivisa dai due mondi. */
export function StatusBarFrame({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-[26px] flex-shrink-0 items-center gap-4 border-t bg-base px-3 font-mono text-[11px] text-muted-foreground">
      {children}
    </div>
  );
}

/** Barra di stato dei terminali: conteggi a sinistra, limiti e plugin installati a destra. */
export default function StatusBar({
  terminalCount,
  attentionCount,
  agentsSummary,
  limits = [],
  hubWaiting = 0,
  onOpenHub,
}: Props) {
  return (
    <StatusBarFrame>
      <span>{terminalCountText(terminalCount)}</span>
      {attentionCount > 0 && <span className="text-status-attention">{attentionCount} in attesa di risposta</span>}
      {hubWaiting > 0 && (
        <button type="button" onClick={onOpenHub} className="text-status-attention hover:underline">
          Agents HUB: {waitingText(hubWaiting)}
        </button>
      )}
      <span className="flex-1" />
      {limits.map((g) => (
        <span key={g.key} className="flex flex-shrink-0 items-center gap-1.5 whitespace-nowrap">
          {g.prefix && <span className="text-faint">{g.prefix}</span>}
          {g.items.map((item, i) => (
            <span key={item.key} className="flex items-center gap-1.5">
              {i > 0 && (
                <span className="text-faint" aria-hidden="true">
                  │
                </span>
              )}
              <span title={item.title} className={cn(SEVERITY_CLASS[item.severity])}>
                {item.text}
              </span>
            </span>
          ))}
        </span>
      ))}
      {agentsSummary && <span className="truncate">{agentsSummary}</span>}
    </StatusBarFrame>
  );
}
