import { describe, it, expect } from "vitest";
import { navigationSections } from "@/data/navigation";
import routerSource from "@/router/index.ts?raw";
import sidebarSource from "@/components/layout/AppSidebar.vue?raw";

describe("navigation", () => {
  const items = navigationSections.flatMap((s) => s.items);

  it("chaque page routée est accessible depuis le menu", () => {
    const routes = [...routerSource.matchAll(/path: "(\/[^"]*)",\s*name/g)].map((m) => m[1]);
    const navRoutes = new Set(items.map((i) => i.route));
    expect(routes.filter((r) => !navRoutes.has(r))).toEqual([]);
  });

  it("ids et routes uniques", () => {
    expect(new Set(items.map((i) => i.id)).size).toBe(items.length);
    expect(new Set(items.map((i) => i.route)).size).toBe(items.length);
  });

  it("chaque icône est connue de la barre latérale", () => {
    for (const i of items) expect(sidebarSource, i.icon).toContain(`"${i.icon}"`.replace(/^"([a-z]+)"$/, "$1:"));
  });
});
