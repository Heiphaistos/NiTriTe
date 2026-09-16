import { describe, it, expect, vi, beforeEach } from "vitest";

const check = vi.fn();
const relaunch = vi.fn();
const ask = vi.fn();
const message = vi.fn();
const invoke = vi.fn();

vi.mock("@tauri-apps/plugin-updater", () => ({ check: () => check() }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: () => relaunch() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  ask: (...a: unknown[]) => ask(...a),
  message: (...a: unknown[]) => message(...a),
}));
vi.mock("@/utils/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));
vi.mock("@/utils/invoke", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { checkForUpdate } from "@/composables/useAutoUpdate";

// import.meta.env.DEV vaut true sous vitest : la garde dev couperait tout.
beforeEach(() => {
  vi.stubEnv("DEV", false);
  vi.clearAllMocks();
});

/** Rend `est_installee` puis route les autres commandes vers `reste`. */
function commandes(installee: boolean, reste: Record<string, unknown> = {}) {
  invoke.mockImplementation((cmd: string) => {
    if (cmd === "est_installee") return Promise.resolve(installee);
    if (cmd in reste) return Promise.resolve(reste[cmd]);
    return Promise.resolve(undefined);
  });
}

const majPortable = { version: "8.218.0", version_actuelle: "8.217.0", notes: "" };

describe("version installee (installeur NSIS)", () => {
  it("ne dit rien quand il n'y a pas de nouvelle version", async () => {
    commandes(true);
    check.mockResolvedValue(null);
    expect(await checkForUpdate()).toBe(false);
    expect(message).not.toHaveBeenCalled();
  });

  it("installe quand l'utilisateur accepte", async () => {
    const install = vi.fn().mockResolvedValue(undefined);
    commandes(true);
    check.mockResolvedValue({
      version: "8.218.0",
      currentVersion: "8.217.0",
      body: "notes",
      downloadAndInstall: install,
    });
    ask.mockResolvedValue(true);
    expect(await checkForUpdate()).toBe(true);
    expect(install).toHaveBeenCalled();
    // Le plugin arrete l'application lui-meme : relancer ici serait du vent.
    expect(relaunch).not.toHaveBeenCalled();
  });

  it("n'installe rien quand l'utilisateur refuse", async () => {
    const install = vi.fn();
    commandes(true);
    check.mockResolvedValue({
      version: "8.218.0",
      currentVersion: "8.217.0",
      body: "",
      downloadAndInstall: install,
    });
    ask.mockResolvedValue(false);
    expect(await checkForUpdate()).toBe(false);
    expect(install).not.toHaveBeenCalled();
  });
});

describe("version portable", () => {
  it("passe par le chemin portable, jamais par le plugin", async () => {
    commandes(false, { portable_maj_verifier: majPortable });
    ask.mockResolvedValue(true);
    expect(await checkForUpdate()).toBe(true);
    expect(check).not.toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith("portable_maj_appliquer", { maj: majPortable }, 300_000);
    expect(relaunch).toHaveBeenCalled();
  });

  it("ne dit rien quand il n'y a pas de nouvelle version", async () => {
    commandes(false, { portable_maj_verifier: null });
    expect(await checkForUpdate()).toBe(false);
    expect(message).not.toHaveBeenCalled();
  });

  it("n'applique rien quand l'utilisateur refuse", async () => {
    commandes(false, { portable_maj_verifier: majPortable });
    ask.mockResolvedValue(false);
    expect(await checkForUpdate()).toBe(false);
    expect(invoke).not.toHaveBeenCalledWith(
      "portable_maj_appliquer",
      expect.anything(),
      expect.anything(),
    );
    expect(relaunch).not.toHaveBeenCalled();
  });

  it("remonte l'echec sans rien casser quand la signature est refusee", async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === "est_installee") return Promise.resolve(false);
      if (cmd === "portable_maj_verifier") return Promise.resolve(majPortable);
      return Promise.reject(new Error("signature invalide"));
    });
    ask.mockResolvedValue(true);
    expect(await checkForUpdate(false)).toBe(false);
    expect(message).toHaveBeenCalled();
    expect(relaunch).not.toHaveBeenCalled();
  });
});

describe("canal injoignable", () => {
  it("reste silencieux au demarrage", async () => {
    invoke.mockRejectedValue(new Error("reseau"));
    expect(await checkForUpdate()).toBe(false);
    expect(message).not.toHaveBeenCalled();
  });

  it("previent quand la verification est demandee a la main", async () => {
    invoke.mockRejectedValue(new Error("reseau"));
    expect(await checkForUpdate(false)).toBe(false);
    expect(message).toHaveBeenCalled();
  });
});
