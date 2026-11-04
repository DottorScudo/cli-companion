import { memo } from "react";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { useStore } from "../../hooks/useStore";
import { saveEdit } from "./actions";
import { sidebarStore } from "./sidebarState";

/** Rinomina inline del workspace (al posto della sua riga). */
function RenameForm({ id, busy }: { id: string; busy: boolean }) {
  const name = useStore(sidebarStore, (s) => s.editingName);
  return (
    <div className="rounded-[10px] border border-border bg-surface">
      <div className="flex flex-col gap-2 px-2.5 py-2">
        <Input
          value={name}
          onChange={(e) => sidebarStore.setEditingName(e.target.value)}
          placeholder="Nome workspace"
          autoFocus
          className="h-8 text-[13px]"
        />
        <div className="flex flex-row gap-1.5">
          <Button size="sm" disabled={busy} onClick={() => void saveEdit(id)}>
            Salva
          </Button>
          <Button size="sm" variant="secondary" onClick={() => sidebarStore.stopEdit()}>
            Annulla
          </Button>
        </div>
      </div>
    </div>
  );
}

export default memo(RenameForm);
