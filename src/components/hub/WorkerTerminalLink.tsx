import { focusTerminal } from "../../app/actions/view";

/** Link «apri il terminale del worker», dato il tab del worker. */
export default function WorkerTerminalLink({ tab }: { tab: string }) {
  return (
    <button
      type="button"
      onClick={() => focusTerminal(tab)}
      className="self-start text-[11px] text-primary hover:underline"
    >
      apri il terminale del worker
    </button>
  );
}
