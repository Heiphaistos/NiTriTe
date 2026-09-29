/** Cale de `@tauri-apps/api/event` : evenements du backend via WebSocket. */
import { onAgentEvent } from "./agent";

export type UnlistenFn = () => void;
export interface Event<T> {
  event: string;
  id: number;
  payload: T;
}

let seq = 0;

export async function listen<T>(event: string, handler: (e: Event<T>) => void): Promise<UnlistenFn> {
  return onAgentEvent(event, (payload) => handler({ event, id: ++seq, payload: payload as T }));
}

export async function once<T>(event: string, handler: (e: Event<T>) => void): Promise<UnlistenFn> {
  const un = await listen<T>(event, (e) => { un(); handler(e); });
  return un;
}

export async function emit(): Promise<void> {
  /* l'interface n'emet pas vers le backend */
}
