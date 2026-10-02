import { test, expect, type Page } from "@playwright/test";
import { UI_MODELS } from "../src/stores/uiModel";
import { navigationSections } from "../src/data/navigation";

/**
 * Pour chaque modèle × (thème sombre, thème clair) × page clé :
 * - aucune erreur JavaScript hors absence du backend Tauri (attendue ici) ;
 * - pas de défilement horizontal de la fenêtre (rien ne déborde) ;
 * - la page est bien dans l'emplacement de la coque et occupe l'espace ;
 * - la barre d'état et le sélecteur rapide de modèle sont visibles ;
 * - capture pleine fenêtre jointe au rapport (artefact CI).
 */
const THEMES = [
  { id: "nitrite-dark", tone: "dark" },
  { id: "forge-light", tone: "light" },
];
const PAGES = ["/", "/diagnostic", "/missions"];

// Erreurs dues à l'absence de Tauri dans un navigateur : attendues.
const EXPECTED = /invoke|transformCallback|__TAURI|tauri|plugin|Failed to fetch|NetworkError|ERR_/i;

async function open(page: Page, model: string, theme: string, path: string) {
  const errors: string[] = [];
  page.on("pageerror", e => { if (!EXPECTED.test(e.message)) errors.push(e.message); });
  await page.addInitScript(([m, t]) => {
    localStorage.setItem("nitrite-ui-model", m);
    localStorage.setItem("nitrite-theme", t);
    localStorage.setItem("nitrite-perf-mode", "light"); // captures stables (sans animations)
  }, [model, theme]);
  await page.goto(path);
  await page.waitForSelector(".app-content-inner", { timeout: 45_000 });
  await page.waitForTimeout(400);
  return errors;
}

for (const model of UI_MODELS) {
  for (const theme of THEMES) {
    test.describe(`${model.name} · ${theme.tone}`, () => {
      for (const path of PAGES) {
        test(`page ${path}`, async ({ page }, info) => {
          const errors = await open(page, model.id, theme.id, path);
          const html = page.locator("html");
          await expect(html).toHaveAttribute("data-ui-model", model.id);
          await expect(html).toHaveAttribute("data-theme-tone", theme.tone);

          // La zone de page est téléportée dans la coque et occupe l'espace.
          const slot = page.locator(`[data-page-slot="${model.id}"] .app-content`);
          await expect(slot).toBeVisible();
          const box = (await slot.boundingBox())!;
          expect(box.width, "largeur de la zone de page").toBeGreaterThan(600);
          expect(box.height, "hauteur de la zone de page").toBeGreaterThan(300);

          // Rien ne déborde horizontalement de la fenêtre.
          const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
          expect(overflow, "débordement horizontal").toBeLessThanOrEqual(1);

          await expect(page.locator(".status-bar")).toBeVisible();
          await expect(page.locator(".uqs-btn")).toBeVisible();

          await info.attach(`${model.id}-${theme.tone}${path.replace(/\//g, "_")}.png`, {
            body: await page.screenshot(), contentType: "image/png",
          });
          expect(errors, "erreurs JavaScript").toEqual([]);
        });
      }
    });
  }
}

test("chaque entrée du menu ouvre sa page sans erreur (Forge)", async ({ page }) => {
  const errors = await open(page, "forge", "nitrite-dark", "/");
  for (const item of navigationSections.flatMap(s => s.items)) {
    await page.locator(".forge-item", { hasText: item.label }).first().click();
    await expect(page).toHaveURL(new RegExp(`${item.route === "/" ? "/$" : item.route}$`));
    await expect(page.locator(".app-content-inner")).toBeVisible();
  }
  expect(errors).toEqual([]);
});

test("petit écran 1280×720 : aucun débordement, dans chaque modèle", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 });
  for (const model of UI_MODELS) {
    await open(page, model.id, "nitrite-dark", "/diagnostic");
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
    expect(overflow, model.id).toBeLessThanOrEqual(1);
  }
});
