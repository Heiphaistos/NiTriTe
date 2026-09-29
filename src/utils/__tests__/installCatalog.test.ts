import { describe, it, expect } from "vitest";
import catalog from "../../../src-tauri/data/programs.json";
import {
  buildCategories, matchesSearch, primaryMethod, PROFILES, profileApps,
  runQueue, formatEta, CATEGORY_ORDER, type CatalogApp,
} from "@/utils/installCatalog";

const apps = catalog as CatalogApp[];

describe("catalogue programs.json", () => {
  it("n'a aucun id en double (cle Vue + resolution backend par id)", () => {
    const ids = apps.map((a) => a.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("n'a aucun identifiant winget en double", () => {
    const ids = apps.map((a) => a.winget_id?.toLowerCase()).filter(Boolean);
    const dupes = ids.filter((id, i) => ids.indexOf(id) !== i);
    expect(dupes).toEqual([]);
  });

  it("a un nom et une categorie pour chaque app", () => {
    for (const a of apps) {
      expect(a.name.trim()).not.toBe("");
      expect(a.category.trim()).not.toBe("");
    }
  });
});

describe("buildCategories", () => {
  it("affiche TOUTES les categories du catalogue", () => {
    const cats = buildCategories(apps).map((c) => c.id);
    for (const a of apps) expect(cats).toContain(a.category);
  });

  it("respecte l'ordre connu puis ajoute les inconnues a la fin", () => {
    const out = buildCategories([
      { id: "a", name: "A", description: "", category: "Zeta Inconnue" },
      { id: "b", name: "B", description: "", category: "Navigateurs" },
      { id: "c", name: "C", description: "", category: "Outils Essentiels" },
    ]);
    expect(out.map((c) => c.id)).toEqual(["Outils Essentiels", "Navigateurs", "Zeta Inconnue"]);
    expect(CATEGORY_ORDER.length).toBeGreaterThan(20);
  });
});

describe("matchesSearch", () => {
  const app: CatalogApp = { id: "x", name: "Éditeur Pro", description: "Sécurité réseau", category: "Securite", winget_id: "Foo.Bar" };
  it("ignore accents et casse", () => {
    expect(matchesSearch(app, "editeur")).toBe(true);
    expect(matchesSearch(app, "SECURITE")).toBe(true);
  });
  it("cherche aussi dans l'identifiant winget", () => {
    expect(matchesSearch(app, "foo.bar")).toBe(true);
  });
  it("accepte une requete vide", () => {
    expect(matchesSearch(app, "  ")).toBe(true);
  });
});

describe("primaryMethod", () => {
  it("priorise winget, puis Chocolatey, puis URL", () => {
    const base = { id: "i", name: "n", description: "", category: "c" };
    expect(primaryMethod({ ...base, winget_id: "A.B", url: "u" })).toBe("winget");
    expect(primaryMethod({ ...base, choco_id: "a", url: "u" })).toBe("choco");
    expect(primaryMethod({ ...base, url: "u" })).toBe("direct");
    expect(primaryMethod(base)).toBe("auto");
  });
});

describe("PROFILES", () => {
  it.each(PROFILES.map((p) => [p.label, p] as const))("profil %s : chaque app existe dans le catalogue", (_l, p) => {
    expect(profileApps(p, apps).length).toBe(p.wingetIds.length);
  });
});

describe("runQueue", () => {
  it("traite les elements dans l'ordre, un a la fois", async () => {
    let running = 0;
    let maxRunning = 0;
    const order: number[] = [];
    const out = await runQueue([1, 2, 3], async (n) => {
      running++; maxRunning = Math.max(maxRunning, running);
      await new Promise((r) => setTimeout(r, 1));
      order.push(n); running--;
      return n * 2;
    });
    expect(order).toEqual([1, 2, 3]);
    expect(maxRunning).toBe(1);
    expect(out.results.map((r) => r.result)).toEqual([2, 4, 6]);
    expect(out.cancelled).toBe(false);
  });

  it("s'arrete avant l'element suivant quand on annule", async () => {
    let stop = false;
    const out = await runQueue([1, 2, 3], async (n) => { if (n === 1) stop = true; return n; }, { shouldStop: () => stop });
    expect(out.results.map((r) => r.item)).toEqual([1]);
    expect(out.cancelled).toBe(true);
  });

  it("calcule une ETA a partir du 2e element", async () => {
    let t = 0;
    const etas: (number | null)[] = [];
    await runQueue([1, 2, 3], async () => { t += 10_000; }, {
      now: () => t,
      onProgress: (p) => etas.push(p.etaSeconds),
    });
    expect(etas[0]).toBeNull();
    expect(etas[1]).toBe(20); // 10 s/app, 2 restantes
    expect(etas[2]).toBe(10);
  });
});

describe("formatEta", () => {
  it("formate minutes et secondes", () => {
    expect(formatEta(75)).toBe("~1 min 15s restantes");
    expect(formatEta(9)).toBe("~9s restantes");
    expect(formatEta(null)).toBe("");
    expect(formatEta(-1)).toBe("");
  });
});
