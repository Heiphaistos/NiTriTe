import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import cssSource from "@/assets/styles/themes.css?raw";
import { PRESET_THEME_GROUPS, PRESET_THEMES } from "@/stores/themeEditor";
import { CSS_THEMES, VAR_THEMES, themeGallery, totalThemeCount, deriveThemeVars } from "@/utils/themeCatalog";
import { useAppStore } from "@/stores/app";

describe("catalogue de thèmes", () => {
  it("chaque thème CSS a son bloc dans themes.css", () => {
    for (const t of CSS_THEMES) expect(cssSource).toContain(`[data-theme="${t.id}"]`);
  });

  it("chaque id des groupes de l'éditeur existe dans PRESET_THEMES", () => {
    const ids = new Set(PRESET_THEMES.map((p) => p.id));
    for (const g of PRESET_THEME_GROUPS) for (const id of g.ids) expect(ids, id).toContain(id);
  });

  it("aucun id en double dans la galerie", () => {
    const all = themeGallery().flatMap((g) => g.themes.map((t) => t.id));
    expect(new Set(all).size).toBe(all.length);
    expect(all.length).toBe(totalThemeCount());
    expect(totalThemeCount()).toBeGreaterThanOrEqual(60);
  });

  it("chaque preset définit les couleurs de base", () => {
    for (const t of VAR_THEMES) {
      for (const k of ["--bg-primary", "--accent-primary", "--text-primary", "--border"]) expect(t.vars?.[k], `${t.id} ${k}`).toMatch(/^#[0-9a-f]{6}$/i);
    }
  });

  it("dérive les variables muted/glow depuis les couleurs", () => {
    const d = deriveThemeVars({ "--accent-primary": "#ff0000", "--success": "#00ff00" });
    expect(d["--accent-muted"]).toBe("rgba(255, 0, 0, 0.12)");
    expect(d["--success-muted"]).toBe("rgba(0, 255, 0, 0.12)");
  });
});

describe("setTheme", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    localStorage.clear();
    document.documentElement.removeAttribute("style");
  });

  it("applique un preset à variables et le mémorise", () => {
    const store = useAppStore();
    store.setTheme("dracula");
    expect(document.documentElement.getAttribute("data-theme")).toBe("custom");
    expect(document.documentElement.style.getPropertyValue("--bg-primary")).not.toBe("");
    expect(localStorage.getItem("nitrite-theme")).toBe("dracula");
  });

  it("revenir à un thème CSS retire les variables du preset", () => {
    const store = useAppStore();
    store.setTheme("nord");
    store.setTheme("cyber-blue");
    expect(document.documentElement.getAttribute("data-theme")).toBe("cyber-blue");
    expect(document.documentElement.style.getPropertyValue("--bg-primary")).toBe("");
  });

  it("un id inconnu retombe sur le thème par défaut", () => {
    const store = useAppStore();
    store.setTheme("theme-supprime");
    expect(store.theme).toBe("nitrite-dark");
  });
});
