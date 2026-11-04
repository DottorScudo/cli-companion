import { useEffect, useSyncExternalStore } from "react";
import { CircleAlert, TriangleAlert, X } from "lucide-react";
import { Button } from "./ui/button";
import { NOTICE_TTL_MS, noticeStore } from "../lib/notices";
import type { Notice, NoticeLevel } from "../lib/notices";
import { cn } from "../lib/utils";

const LEVEL_STYLE: Record<NoticeLevel, { icon: typeof CircleAlert; border: string; iconClass: string; label: string }> =
  {
    error: { icon: CircleAlert, border: "border-destructive/50", iconClass: "text-destructive", label: "Errore" },
    warning: {
      icon: TriangleAlert,
      border: "border-status-attention/50",
      iconClass: "text-status-attention",
      label: "Attenzione",
    },
  };

function NoticeItem({ notice }: { notice: Notice }) {
  const { icon: Icon, border, iconClass, label } = LEVEL_STYLE[notice.level];

  // Gli avvisi non sticky scompaiono da soli; un avviso ripetuto (nuovo id) riarma il timer.
  useEffect(() => {
    if (notice.sticky) return;
    const timer = setTimeout(() => noticeStore.dismiss(notice.id), NOTICE_TTL_MS);
    return () => clearTimeout(timer);
  }, [notice.id, notice.sticky]);

  return (
    <div
      role={notice.level === "error" ? "alert" : "status"}
      className={cn("pointer-events-auto flex items-start gap-2 rounded-lg border bg-surface p-3 shadow-lg", border)}
    >
      <Icon className={cn("mt-0.5 h-4 w-4 flex-shrink-0", iconClass)} aria-hidden />
      <div className="min-w-0 flex-1">
        <p className="text-xs font-semibold">{label}</p>
        <p className="break-words text-xs text-muted-foreground">{notice.text}</p>
      </div>
      <Button
        size="icon"
        variant="ghost"
        className="h-6 w-6 flex-shrink-0"
        title="Chiudi"
        aria-label="Chiudi l’avviso"
        onClick={() => noticeStore.dismiss(notice.id)}
      >
        <X />
      </Button>
    </div>
  );
}

/** Area degli avvisi (errori e avvisi di salvataggio) in basso a destra: non copre i terminali e non blocca l'interfaccia. */
export default function NoticeCenter() {
  const notices = useSyncExternalStore(noticeStore.subscribe, noticeStore.getSnapshot);
  if (notices.length === 0) return null;
  return (
    <div
      aria-label="Avvisi"
      className="pointer-events-none fixed bottom-9 right-3 z-[60] flex w-[360px] max-w-[calc(100vw-1.5rem)] flex-col gap-2"
    >
      {notices.map((n) => (
        <NoticeItem key={n.id} notice={n} />
      ))}
    </div>
  );
}
