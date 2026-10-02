import { describe, it, expect, beforeEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { BUILTIN_MISSIONS } from "@/data/missions";
import { navigationSections } from "@/data/navigation";
import { useMissionStore, missionReport } from "@/stores/mission";

const routes = new Set(navigationSections.flatMap(s => s.items.map(i => i.route)));

beforeEach(() => { localStorage.clear(); setActivePinia(createPinia()); });

describe("modèles de mission", () => {
  it("chaque étape ouvre une page existante du menu", () => {
    for (const t of BUILTIN_MISSIONS) {
      expect(t.steps.length, t.id).toBeGreaterThan(1);
      for (const s of t.steps) expect(routes.has(s.route), `${t.id} ${s.route}`).toBe(true);
    }
  });
  it("ids uniques", () => {
    expect(new Set(BUILTIN_MISSIONS.map(t => t.id)).size).toBe(BUILTIN_MISSIONS.length);
  });
});

describe("store mission", () => {
  it("démarre, enchaîne les étapes et termine", () => {
    const m = useMissionStore();
    const first = m.start("reseau", "  PC de Julie ");
    expect(first?.route).toBe("/network");
    expect(m.active?.client).toBe("PC de Julie");
    expect(m.complete("done")?.route).toBe("/dns-switcher");
    expect(m.complete("skipped")?.route).toBe("/wifi-analyzer");
    expect(m.progress).toBe(40);
    m.complete(); m.complete();
    expect(m.complete()).toBeNull();
    expect(m.isFinished).toBe(true);
    expect(m.active?.finishedAt).toBeTruthy();
  });

  it("refaire une étape la remet à faire et devient l'étape courante", () => {
    const m = useMissionStore();
    m.start("sauvegarde");
    m.complete(); m.complete(); m.complete();
    m.reopen(1);
    expect(m.isFinished).toBe(false);
    expect(m.currentStep?.route).toBe("/hash-checker");
    expect(m.active?.finishedAt).toBeUndefined();
  });

  it("persiste la mission en cours et l'archive à la clôture", async () => {
    const m = useMissionStore();
    m.start("pc-lent");
    m.setNote(0, "Chrome à 80 % CPU");
    await Promise.resolve();
    await new Promise(r => setTimeout(r, 0));
    expect(JSON.parse(localStorage.getItem("nitrite-mission-active")!).templateId).toBe("pc-lent");
    m.stop();
    expect(m.active).toBeNull();
    expect(m.history[0].steps[0].note).toBe("Chrome à 80 % CPU");
  });

  it("modèles personnalisés : création, filtrage des routes inconnues, suppression", () => {
    const m = useMissionStore();
    expect(m.saveTemplate("", "", ["/diagnostic"])).toBeNull();
    expect(m.saveTemplate("Vide", "", ["/nexistepas"])).toBeNull();
    const t = m.saveTemplate("Révision", "annuelle", ["/diagnostic", "/nexistepas", "/cleaner"])!;
    expect(t.steps.map(s => s.route)).toEqual(["/diagnostic", "/cleaner"]);
    expect(m.templates.some(x => x.id === t.id)).toBe(true);
    m.deleteTemplate(t.id);
    expect(m.templates.some(x => x.id === t.id)).toBe(false);
  });

  it("rapport : statut, notes et récapitulatif", () => {
    const m = useMissionStore();
    m.start("sauvegarde", "Poste compta");
    m.setNote(0, "48 Go copiés");
    m.complete("done"); m.complete("skipped");
    const r = missionReport(m.active!).join("\n");
    expect(r).toContain("RAPPORT D'INTERVENTION — Sauvegarde & migration");
    expect(r).toContain("Client / poste : Poste compta");
    expect(r).toContain("1. [fait] Sauvegarde");
    expect(r).toContain("Note : 48 Go copiés");
    expect(r).toContain("2. [passé] Hash Checker");
    expect(r).toContain("3. [à faire] Clonage Système");
    expect(r).toContain("1 étape(s) faite(s) sur 3.");
  });
});
