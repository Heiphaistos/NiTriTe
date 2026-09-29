/** Cale de `@tauri-apps/api/path` (chemins du poste ou tourne l'agent). */
import { agentFetch } from "./agent";

export async function homeDir(): Promise<string> {
  return (await agentFetch<string | null>("/host/path/home", undefined, "GET")) ?? "C:\\";
}

export async function join(...parts: string[]): Promise<string> {
  return parts
    .filter((p) => p !== "")
    .map((p, i) => (i === 0 ? p.replace(/[\\/]+$/, "") : p.replace(/^[\\/]+|[\\/]+$/g, "")))
    .join("\\");
}

export async function documentDir(): Promise<string> {
  return join(await homeDir(), "Documents");
}

export async function downloadDir(): Promise<string> {
  return join(await homeDir(), "Downloads");
}

export async function desktopDir(): Promise<string> {
  return join(await homeDir(), "Desktop");
}

export const sep = () => "\\";
