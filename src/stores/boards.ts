// Code dei task per workspace (pannello Task, sidebar, Tasks Manager) e
// errori di caricamento, mostrati solo nel Tasks Manager.

import { createStore } from "../lib/store.ts";
import { mergeBoard, taskCounts } from "../lib/board.ts";
import type { BoardView, TaskCounts } from "../lib/board.ts";

export interface BoardsState {
  boards: Readonly<Record<string, BoardView>>;
  errors: Readonly<Record<string, string>>;
}

export const INITIAL_BOARDS: BoardsState = { boards: {}, errors: {} };

const has = (record: object, key: string): boolean => Object.prototype.hasOwnProperty.call(record, key);

/** Memorizza una board; una più vecchia di quella nota (rev) non la sostituisce e non notifica. */
export function storeBoard(s: BoardsState, board: BoardView): BoardsState {
  const current = s.boards[board.workspace_id];
  const merged = mergeBoard(current, board);
  return merged === current ? s : { ...s, boards: { ...s.boards, [board.workspace_id]: merged } };
}

export function setBoardError(s: BoardsState, workspaceId: string, message: string): BoardsState {
  return s.errors[workspaceId] === message ? s : { ...s, errors: { ...s.errors, [workspaceId]: message } };
}

export function clearBoardError(s: BoardsState, workspaceId: string): BoardsState {
  if (!has(s.errors, workspaceId)) return s;
  const { [workspaceId]: _removed, ...rest } = s.errors;
  return { ...s, errors: rest };
}

const countsCache = new WeakMap<BoardsState["boards"], Record<string, TaskCounts>>();

/** Contatori dei task di ogni board (sidebar), memoizzati per oggetto `boards`. */
export function taskCountsOf(boards: BoardsState["boards"]): Record<string, TaskCounts> {
  let counts = countsCache.get(boards);
  if (!counts) {
    counts = Object.fromEntries(Object.entries(boards).map(([id, b]) => [id, taskCounts(b)]));
    countsCache.set(boards, counts);
  }
  return counts;
}

export function createBoardsStore() {
  const store = createStore<BoardsState>(INITIAL_BOARDS);
  return {
    ...store,
    store: (board: BoardView) => store.update((s) => storeBoard(s, board)),
    setError: (workspaceId: string, message: string) => store.update((s) => setBoardError(s, workspaceId, message)),
    clearError: (workspaceId: string) => store.update((s) => clearBoardError(s, workspaceId)),
  };
}

export const boardsStore = createBoardsStore();
