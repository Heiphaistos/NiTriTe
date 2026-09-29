/** Cale de `@tauri-apps/api/window` : pas de fenetre native, l'onglet en tient lieu. */
const noop = async () => () => {};

export function getCurrentWindow() {
  return {
    label: "main",
    listen: noop,
    once: noop,
    onCloseRequested: noop,
    destroy: async () => {},
    close: async () => { window.close(); },
    minimize: async () => {},
    maximize: async () => {},
    unmaximize: async () => {},
    isMaximized: async () => false,
    setTitle: async (t: string) => { document.title = t; },
    setFocus: async () => { window.focus(); },
  };
}

export const getCurrent = getCurrentWindow;
