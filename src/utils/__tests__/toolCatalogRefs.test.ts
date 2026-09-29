import { describe, it, expect } from "vitest";
import src from "@/components/shared/DiagnosticToolsGrid.vue?raw";
import tools from "../../../src-tauri/data/tools.json";

// Les boutons qui lancent un outil du catalogue le designent par nom + section
// (launch_tool). Un renommage dans tools.json casserait le bouton sans erreur
// de compilation : ce test relie les deux.
describe("references au catalogue tools.json", () => {
  const refs = [...src.matchAll(/catalog: \{ name: "([^"]+)", section: "([^"]+)" \}/g)].map((m) => [m[1], m[2]] as const);

  it("trouve les references", () => {
    expect(refs.length).toBeGreaterThan(0);
  });

  it.each(refs)("%s (%s) existe", (name, section) => {
    expect((tools as { name: string; section: string }[]).some((t) => t.name === name && t.section === section)).toBe(true);
  });
});
