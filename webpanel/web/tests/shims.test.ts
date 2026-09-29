import { describe, it, expect, vi, beforeEach } from "vitest";
import { captureTokenFromUrl, agentToken, agentFetch, AgentError } from "../shims/agent";
import { join } from "../shims/path";
import { invoke } from "../shims/core";

beforeEach(() => {
  sessionStorage.clear();
  vi.restoreAllMocks();
});

describe("jeton de session", () => {
  it("est lu dans le fragment puis retiré de l'URL", () => {
    history.replaceState(null, "", "/settings#t=0123456789abcdef0123");
    captureTokenFromUrl();
    expect(agentToken()).toBe("0123456789abcdef0123");
    expect(location.hash).toBe("");
    expect(location.pathname).toBe("/settings");
  });

  it("ignore un fragment sans jeton", () => {
    history.replaceState(null, "", "/#section");
    captureTokenFromUrl();
    expect(agentToken()).toBe("");
  });
});

describe("invoke", () => {
  it("POST /api/invoke/<commande> avec le jeton et les arguments", async () => {
    sessionStorage.setItem("nitrite-agent-token", "tok");
    const spy = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify([1, 2]), { status: 200 }));
    const out = await invoke<number[]>("install_app", { appId: "7zip" });
    expect(out).toEqual([1, 2]);
    const [url, init] = spy.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/invoke/install_app");
    expect((init.headers as Record<string, string>)["X-Nitrite-Token"]).toBe("tok");
    expect(JSON.parse(init.body as string)).toEqual({ appId: "7zip" });
  });

  it("rejette avec l'erreur sérialisée du backend, comme Tauri", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify("Commande refusée"), { status: 422 }));
    await expect(invoke("x")).rejects.toMatchObject({ status: 422, payload: "Commande refusée" });
  });

  it("signale un agent injoignable", async () => {
    vi.spyOn(globalThis, "fetch").mockRejectedValue(new TypeError("Failed to fetch"));
    await expect(agentFetch("/health", undefined, "GET")).rejects.toBeInstanceOf(AgentError);
  });
});

describe("path.join", () => {
  it("assemble des chemins Windows sans séparateurs doublés", async () => {
    expect(await join("C:\\Users\\Bob\\", "Documents", "NiTriTe\\")).toBe("C:\\Users\\Bob\\Documents\\NiTriTe");
  });
});

describe("menu Agent", () => {
  it("formate la durée d'activité", async () => {
    const { formatUptime } = await import("../shims/agent-menu");
    expect(formatUptime(59)).toBe("0 min");
    expect(formatUptime(3 * 60)).toBe("3 min");
    expect(formatUptime(2 * 3600 + 5 * 60)).toBe("2 h 5 min");
  });

  it("affiche la pastille et les infos de l'agent, dont la mise à jour", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify({
      version: "1.1.0", nitriteVersion: "8.221.0", port: 7878, lan: false, clients: 1,
      idleMinutes: 15, uptimeSeconds: 120, update: { version: "1.2.0", url: "https://example.test/r" }, logPath: "C:\\x\\agent.log",
    }), { status: 200 }));
    const { mountAgentMenu } = await import("../shims/agent-menu");
    mountAgentMenu();
    const pill = [...document.querySelectorAll("button")].find((b) => b.textContent?.includes("Agent"))!;
    expect(pill).toBeTruthy();
    pill.click();
    await vi.waitFor(() => expect(document.querySelector('[role="dialog"]')).toBeTruthy());
    const text = document.querySelector('[role="dialog"]')!.textContent!;
    expect(text).toContain("v1.1.0");
    expect(text).toContain("v8.221.0");
    expect(text).toContain("Nouvelle version disponible : v1.2.0");
    expect((document.querySelector('[role="dialog"] a') as HTMLAnchorElement).rel).toContain("noopener");
  });
});

describe("authentification par cookie", () => {
  it("n'envoie pas d'en-tête de jeton quand l'onglet n'en a pas (cookie de session)", async () => {
    const spy = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("null", { status: 200 }));
    await invoke("get_apps");
    const init = spy.mock.calls[0][1] as RequestInit;
    expect((init.headers as Record<string, string>)["X-Nitrite-Token"]).toBeUndefined();
  });
});
