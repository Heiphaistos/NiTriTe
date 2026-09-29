/** Cale de `@tauri-apps/plugin-fs` (fichiers du poste ou tourne l'agent). */
import { agentFetch } from "./agent";

export async function readTextFile(path: string): Promise<string> {
  return agentFetch<string>("/host/fs/read-text", { path });
}

export async function writeTextFile(path: string, contents: string): Promise<void> {
  await agentFetch("/host/fs/write-text", { path, contents });
}

export async function mkdir(path: string, o?: { recursive?: boolean }): Promise<void> {
  await agentFetch("/host/fs/mkdir", { path, recursive: o?.recursive ?? false });
}

export const BaseDirectory = {} as Record<string, number>;
