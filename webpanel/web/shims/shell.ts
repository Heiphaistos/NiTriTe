/** Cale de `@tauri-apps/plugin-shell` / `plugin-opener` : liens dans un nouvel onglet. */
export async function open(url: string): Promise<void> {
  window.open(url, "_blank", "noopener");
}

export const openUrl = open;
export const openPath = open;
