import { FolderOpen } from "lucide-react";
import { openFolder } from "./actions";
import { ICON_BTN } from "./styles";

export default function OpenFolderButton({ id }: { id: string }) {
  return (
    <button
      type="button"
      className={ICON_BTN}
      title="Apri cartella"
      aria-label="Apri cartella"
      onClick={() => openFolder(id)}
    >
      <FolderOpen />
    </button>
  );
}
