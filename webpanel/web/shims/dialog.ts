/**
 * Cale de `@tauri-apps/plugin-dialog`.
 * - confirm/ask/message : boites du navigateur (meme poste que l'utilisateur) ;
 * - open/save : vraie boite de fichiers Windows, ouverte par l'agent.
 */
import { agentFetch } from "./agent";

type Opts = string | { title?: string; kind?: string; okLabel?: string; cancelLabel?: string };

function prefix(o?: Opts): string {
  const t = typeof o === "string" ? o : o?.title;
  return t && t !== "Nitrite" && t !== "NiTriTe" ? `${t}\n\n` : "";
}

export async function confirm(msg: string, o?: Opts): Promise<boolean> {
  return window.confirm(prefix(o) + msg);
}

export const ask = confirm;

export async function message(msg: string, o?: Opts): Promise<void> {
  window.alert(prefix(o) + msg);
}

export interface DialogFilter { name: string; extensions: string[] }
export interface OpenDialogOptions { title?: string; defaultPath?: string; filters?: DialogFilter[]; multiple?: boolean; directory?: boolean }
export interface SaveDialogOptions { title?: string; defaultPath?: string; filters?: DialogFilter[] }

export async function open(o: OpenDialogOptions = {}): Promise<string | string[] | null> {
  return agentFetch<string | string[] | null>("/host/dialog/open", o);
}

export async function save(o: SaveDialogOptions = {}): Promise<string | null> {
  return agentFetch<string | null>("/host/dialog/save", o);
}
