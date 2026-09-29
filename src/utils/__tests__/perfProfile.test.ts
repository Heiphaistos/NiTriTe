import { describe, it, expect } from "vitest";
import {
  detectTier, resolveTier, startupConcurrency, pollMultiplier, runLimited, scaledInterval,
} from "@/utils/perfProfile";

describe("detectTier", () => {
  it("passe en léger sur un double cœur ou 4 Go", () => {
    expect(detectTier({ cores: 2, memoryGb: 8, reducedMotion: false })).toBe("light");
    expect(detectTier({ cores: 8, memoryGb: 4, reducedMotion: false })).toBe("light");
  });
  it("équilibré sur un quadri-cœur ou si l'OS réduit les animations", () => {
    expect(detectTier({ cores: 4, memoryGb: 8, reducedMotion: false })).toBe("balanced");
    expect(detectTier({ cores: 12, memoryGb: 8, reducedMotion: true })).toBe("balanced");
  });
  it("complet sur une machine confortable, même sans deviceMemory", () => {
    expect(detectTier({ cores: 8, memoryGb: 8, reducedMotion: false })).toBe("full");
    expect(detectTier({ cores: 16, memoryGb: null, reducedMotion: false })).toBe("full");
  });
  it("le choix manuel l'emporte sur la détection", () => {
    const weak = { cores: 2, memoryGb: 2, reducedMotion: false };
    expect(resolveTier("full", weak)).toBe("full");
    expect(resolveTier("auto", weak)).toBe("light");
  });
});

describe("réglages dérivés", () => {
  it("limite les sondes et allonge les sondages en léger", () => {
    expect(startupConcurrency("light")).toBe(2);
    expect(startupConcurrency("full")).toBeGreaterThan(startupConcurrency("balanced"));
    expect(pollMultiplier("light")).toBe(2);
    expect(pollMultiplier("full")).toBe(1);
  });
  it("scaledInterval lit le profil posé sur <html>", () => {
    document.documentElement.setAttribute("data-perf", "light");
    expect(scaledInterval(3000)).toBe(6000);
    document.documentElement.setAttribute("data-perf", "full");
    expect(scaledInterval(3000)).toBe(3000);
    document.documentElement.removeAttribute("data-perf");
  });
});

describe("runLimited", () => {
  it("ne dépasse jamais la limite et garde l'ordre des résultats", async () => {
    let inFlight = 0, peak = 0;
    const tasks = [30, 5, 20, 1, 10].map((ms, i) => async () => {
      inFlight++; peak = Math.max(peak, inFlight);
      await new Promise((r) => setTimeout(r, ms));
      inFlight--;
      if (i === 3) throw new Error("boom");
      return i;
    });
    const out = await runLimited(tasks, 2);
    expect(peak).toBe(2);
    expect(out.map((r) => r.status)).toEqual(["fulfilled", "fulfilled", "fulfilled", "rejected", "fulfilled"]);
    expect((out[4] as PromiseFulfilledResult<number>).value).toBe(4);
  });
  it("accepte une liste vide", async () => {
    expect(await runLimited([], 3)).toEqual([]);
  });
});
