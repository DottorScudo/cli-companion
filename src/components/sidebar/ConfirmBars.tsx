import { memo } from "react";
import { Button } from "../ui/button";
import { confirmArchive, confirmRemove } from "./actions";
import { sidebarStore } from "./sidebarState";

/** Conferma della rimozione del workspace. */
export const RemoveConfirm = memo(function RemoveConfirm({ id, busy }: { id: string; busy: boolean }) {
  return (
    <div className="flex items-center gap-1.5 px-2.5 pb-1.5 pt-1">
      <span className="flex-1 text-xs">Rimuovere?</span>
      <Button size="sm" variant="destructive" disabled={busy} onClick={() => void confirmRemove(id)}>
        Sì
      </Button>
      <Button size="sm" variant="secondary" onClick={() => sidebarStore.cancelRemove()}>
        No
      </Button>
    </div>
  );
});

interface ArchiveProps {
  id: string;
  name: string;
  /** Terminali aperti: verranno chiusi. */
  terminals: number;
  busy: boolean;
}

/** Conferma dell'archiviazione: chiude i terminali e ferma la coda. */
export const ArchiveConfirm = memo(function ArchiveConfirm({ id, name, terminals, busy }: ArchiveProps) {
  return (
    <div className="flex flex-col gap-1.5 px-2.5 pb-1.5 pt-1">
      <span className="text-xs">
        Archiviare {name}?{terminals > 0 && ` Verranno chiusi i suoi ${terminals} terminali aperti e`}
        {terminals > 0 ? " la" : " La"} coda resterà ferma finché non lo ripristini.
      </span>
      <div className="flex items-center gap-1.5">
        <Button size="sm" disabled={busy} onClick={() => void confirmArchive(id)}>
          Sì
        </Button>
        <Button size="sm" variant="secondary" onClick={() => sidebarStore.cancelArchive()}>
          No
        </Button>
      </div>
    </div>
  );
});
