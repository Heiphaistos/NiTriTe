/** Cale de `@tauri-apps/api/core` : `invoke` -> POST /api/invoke/<commande>. */
import { agentFetch } from "./agent";
import "./banner";

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return agentFetch<T>(`/invoke/${encodeURIComponent(cmd)}`, args ?? {});
}

export function isTauri(): boolean {
  return true;
}

export function convertFileSrc(path: string): string {
  return path;
}

export function transformCallback(): number {
  return 0;
}
