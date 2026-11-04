import { useCallback } from "react";

/**
 * Ref che imposta `inert` come solo attributo (niente remount): ciò che è
 * coperto dal Tasks Manager o dall'Ufficio non resta raggiungibile con Tab.
 */
export function useInertRef(inert: boolean): (el: HTMLElement | null) => void {
  return useCallback(
    (el: HTMLElement | null) => {
      if (!el) return;
      if (inert) el.setAttribute("inert", "");
      else el.removeAttribute("inert");
    },
    [inert],
  );
}
