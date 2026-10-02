import { defineConfig, devices } from "@playwright/test";

/**
 * Tests visuels des modèles d'interface : l'application web (sans le backend
 * Tauri) est servie par `vite preview`, puis chaque modèle est ouvert avec un
 * thème sombre et un thème clair. Les captures sont jointes au rapport et
 * publiées comme artefact de la CI.
 */
export default defineConfig({
  testDir: "e2e",
  timeout: 60_000,
  fullyParallel: true,
  retries: 0,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: "http://127.0.0.1:4173",
    viewport: { width: 1440, height: 900 },
    ...devices["Desktop Chrome"],
    // Navigateur pré-installé dans certains environnements (sinon celui de Playwright).
    launchOptions: process.env.PW_CHROMIUM ? { executablePath: process.env.PW_CHROMIUM } : {},
  },
  webServer: {
    command: "npx vite build && npx vite preview --port 4173 --strictPort --host 127.0.0.1",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
  },
});
