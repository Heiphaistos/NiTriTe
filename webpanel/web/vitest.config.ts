import { defineConfig } from "vitest/config";

export default defineConfig({
  // Motif relatif a ce dossier : un chemin absolu contient des « \ » sous
  // Windows et ne correspond alors a aucun fichier (echec en CI Windows).
  root: __dirname,
  test: {
    environment: "happy-dom",
    include: ["tests/**/*.test.ts"],
  },
});
