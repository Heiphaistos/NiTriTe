/** Cale de `@tauri-apps/plugin-process`. */
import { agentFetch } from "./agent";

export async function relaunch(): Promise<void> {
  location.reload();
}

export async function exit(): Promise<void> {
  await agentFetch("/host/shutdown");
}
