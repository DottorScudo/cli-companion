import { memo } from "react";
import { useStore } from "../../hooks/useStore";
import { useOverlayShown } from "../../hooks/useView";
import { shallowEqual } from "../../lib/store";
import { layoutStore, shownPaneLayouts } from "../../stores/layout";
import { tabsStore } from "../../stores/tabs";
import Logo from "../Logo";
import TerminalPane from "./TerminalPane";
import { useInertRef } from "./useInertRef";

const tabKeys = (tabs: ReturnType<typeof tabsStore.getState>) => tabs.map((t) => t.key);

const anyPaneVisible = (s: ReturnType<typeof layoutStore.getState>) =>
  Object.values(shownPaneLayouts(s)).some((p) => p.visible);

/**
 * Area dei pannelli. TUTTI i TerminalView di TUTTI i gruppi sono figli
 * diretti di questo contenitore, con chiave stabile e nell'ordine di
 * creazione: layout, riordino, cambio gruppo e maximize cambiano solo
 * lo stile (rettangolo / display), mai la struttura del DOM, quindi
 * nessun terminale viene smontato o spostato (ucciderebbe il pty).
 * Il contenitore interno e' fisso (inset 3px + 3px per pannello =
 * 6px di margine e di spazio fra i pannelli).
 *
 * Si iscrive alle sole chiavi dei tab: ogni pannello legge da sé il resto.
 */
function TerminalGrid() {
  const keys = useStore(tabsStore, tabKeys, shallowEqual);
  const visible = useStore(layoutStore, anyPaneVisible);
  const overlayShown = useOverlayShown();
  const inertRef = useInertRef(overlayShown);

  return (
    <div className="relative min-h-0 min-w-0 flex-1 bg-pane-area">
      <div
        className="absolute inset-[3px]"
        aria-hidden={overlayShown || undefined}
        // `inert` solo come attributo (niente remount): i terminali
        // nascosti dal Tasks Manager non restano raggiungibili con Tab.
        ref={inertRef}
      >
        {!visible && (
          <div className="absolute inset-[3px] flex flex-col items-center justify-center gap-3 rounded-[10px] border bg-pane-area p-5">
            <Logo size={40} />
            <span className="text-[13px] font-medium">Nessun terminale aperto</span>
            <span className="text-center text-xs text-muted-foreground">
              Apri un terminale da un workspace, o con un avvio dal menu ⋯
            </span>
          </div>
        )}
        {keys.map((key) => (
          <TerminalPane key={key} tabId={key} />
        ))}
      </div>
    </div>
  );
}

export default memo(TerminalGrid);
