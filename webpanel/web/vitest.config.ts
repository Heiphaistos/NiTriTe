import { defineConfig } from "vitest/config";
import { resolve } from "path";

export default defineConfig({
  test: {
    environment: "happy-dom",
    include: [resolve(__dirname, "tests/**/*.test.ts")],
  },
});
