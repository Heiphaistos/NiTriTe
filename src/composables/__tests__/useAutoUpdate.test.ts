import { describe, it, expect, vi, beforeEach } from "vitest";

const check = vi.fn();
const relaunch = vi.fn();
const ask = vi.fn();
const message = vi.fn();

vi.mock("@tauri-apps/plugin-updater", () => ({ check: () => check() }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: () => relaunch() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  ask: (...a: unknown[]) => ask(...a),
  message: (...a: unknown[]) => message(...a),
}));
vi.mock("@/utils/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

const invoke = vi.fn();
vi.mock("@/utils/invoke", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { checkForUpdate } from "@/composables/useAutoUpdate";

// import.meta.env.DEV vaut true sous vitest : la garde dev coupe tout.
// On la neutralise pour exercer la vraie logique.
beforeEach(() => {
  vi.stubEnv("DEV", false);
  vi.clearAllMocks();
  invoke.mockResolvedValue(false);   // installation NSIS par defaut
});

const majDispo = (downloadAndInstall = vi.fn()) => ({
  version: "9.0.0",
  currentVersion: "8.215.0",
  body: "notes",
  downloadAndInstall,
});

describe("checkForUpdate", () => {
  it("ne fait rien quand aucune mise a jour n'est annoncee", async () => {
    check.mockResolvedValue(null);
    expect(await checkForUpdate()).toBe(false);
    expect(message).not.toHaveBeenCalled();
  });

  it("installe et redemarre quand l'utilisateur accepte", async () => {
    const install = vi.fn().mockResolvedValue(undefined);
    check.mockResolvedValue(majDispo(install));
    ask.mockResolvedValue(true);
    expect(await checkForUpdate()).toBe(true);
    expect(install).toHaveBeenCalled();
    expect(relaunch).toHaveBeenCalled();
  });

  it("n'installe rien quand l'utilisateur refuse", async () => {
    const install = vi.fn();
    check.mockResolvedValue(majDispo(install));
    ask.mockResolvedValue(false);
    expect(await checkForUpdate()).toBe(false);
    expect(install).not.toHaveBeenCalled();
    expect(relaunch).not.toHaveBeenCalled();
  });

  it("ne propose rien a une copie portable : l'installeur NSIS n'a pas le contenu", async () => {
    invoke.mockResolvedValue(true);
    check.mockResolvedValue(majDispo());
    expect(await checkForUpdate()).toBe(false);
    expect(check).not.toHaveBeenCalled();
    expect(message).not.toHaveBeenCalled();
  });

  it("reste silencieux quand le reseau tombe au demarrage", async () => {
    check.mockRejectedValue(new Error("reseau"));
    expect(await checkForUpdate()).toBe(false);
    expect(message).not.toHaveBeenCalled();
  });

  it("previent quand la verification est demandee a la main", async () => {
    check.mockRejectedValue(new Error("reseau"));
    expect(await checkForUpdate(false)).toBe(false);
    expect(message).toHaveBeenCalled();
  });
});
