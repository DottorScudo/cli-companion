import { useRef, useState } from "react";
import type { DragEvent } from "react";

/** Il tipo del `DataTransfer` quando si trascinano file dal sistema. */
const FILES_TYPE = "Files";

function carriesFiles(e: DragEvent<HTMLElement>): boolean {
  return e.dataTransfer.types.includes(FILES_TYPE);
}

/**
 * Zona di rilascio di file (con `dragDropEnabled: false` il webview riceve gli
 * eventi HTML e i `File`, senza percorso). `dragging` evidenzia la zona; il
 * contatore regge entrate e uscite dai figli. Trascinamenti di altro (testo,
 * task) non la toccano. Con `enabled` falso il rilascio è rifiutato.
 */
export function useFileDrop(onFiles: (files: File[]) => void, enabled: boolean) {
  const [dragging, setDragging] = useState(false);
  const depth = useRef(0);

  const reset = () => {
    depth.current = 0;
    setDragging(false);
  };

  return {
    dragging,
    handlers: {
      onDragEnter: (e: DragEvent<HTMLElement>) => {
        if (!carriesFiles(e)) return;
        e.preventDefault();
        depth.current += 1;
        setDragging(true);
      },
      onDragOver: (e: DragEvent<HTMLElement>) => {
        if (!carriesFiles(e)) return;
        e.preventDefault();
        e.dataTransfer.dropEffect = enabled ? "copy" : "none";
      },
      onDragLeave: (e: DragEvent<HTMLElement>) => {
        if (!carriesFiles(e)) return;
        depth.current = Math.max(0, depth.current - 1);
        if (depth.current === 0) setDragging(false);
      },
      onDrop: (e: DragEvent<HTMLElement>) => {
        if (!carriesFiles(e)) return;
        e.preventDefault();
        reset();
        if (enabled) onFiles(Array.from(e.dataTransfer.files));
      },
    },
  };
}
