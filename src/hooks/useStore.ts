import { useCallback, useEffect, useMemo, useRef, useSyncExternalStore } from "react";
import type { Store } from "../lib/store.ts";

type Equal<S> = (a: S, b: S) => boolean;

const identity = <T>(value: T): T => value;

const sameRefs = (a: readonly unknown[], b: readonly unknown[]): boolean =>
  a.length === b.length && a.every((v, i) => Object.is(v, b[i]));

/**
 * Valore derivato da uno o più archivi, con `useSyncExternalStore` (nessun
 * tearing). `stores` deve essere stabile (costante di modulo o memoizzato).
 *
 * `compute` rigira solo se cambia il riferimento dello stato di un archivio o
 * la funzione stessa. Se il nuovo valore è `isEqual` al precedente, si
 * restituisce il precedente: il riferimento stabile è ciò che evita il
 * ridisegno dei figli memoizzati. Con `isEqual` inline `compute` rigira a ogni
 * render: per i valori derivati conviene una funzione di confronto di modulo.
 */
export function useStores<S>(stores: readonly Store<unknown>[], compute: () => S, isEqual: Equal<S> = Object.is): S {
  // Ultima selezione resa: tiene il riferimento anche quando cambia `compute`
  // (selettore inline) e la cache qui sotto riparte da zero.
  const last = useRef<{ selection: S } | null>(null);

  const subscribe = useCallback(
    (onChange: () => void) => {
      const offs = stores.map((s) => s.subscribe(onChange));
      return () => offs.forEach((off) => off());
    },
    [stores],
  );

  // `useSyncExternalStore` pretende che lo snapshot sia lo stesso oggetto
  // finché nulla cambia: la cache della chiusura lo garantisce.
  const getSnapshot = useMemo(() => {
    let cache: { states: unknown[]; selection: S } | null = null;
    return (): S => {
      const states = stores.map((s) => s.getState());
      if (cache !== null && sameRefs(cache.states, states)) return cache.selection;
      const next = compute();
      const previous = cache ?? last.current;
      const selection = previous !== null && isEqual(previous.selection, next) ? previous.selection : next;
      cache = { states, selection };
      return selection;
    };
  }, [stores, compute, isEqual]);

  const value = useSyncExternalStore(subscribe, getSnapshot);
  useEffect(() => {
    last.current = { selection: value };
  }, [value]);
  return value;
}

/**
 * Stato di un archivio, o una sua selezione (`selector`): il componente si
 * ridisegna solo quando la selezione cambia secondo `isEqual` (default
 * `Object.is`; per array e oggetti piccoli passare `shallowEqual`).
 */
export function useStore<T>(store: Store<T>): T;
export function useStore<T, S>(store: Store<T>, selector: (state: T) => S, isEqual?: Equal<S>): S;
export function useStore<T, S>(store: Store<T>, selector?: (state: T) => S, isEqual?: Equal<S>): S | T {
  const stores = useMemo(() => [store], [store]);
  const select = selector ?? (identity as (state: T) => S | T);
  const compute = useCallback(() => select(store.getState()), [store, select]);
  return useStores(stores, compute, isEqual as Equal<S | T> | undefined);
}
