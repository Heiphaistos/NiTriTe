import { describe, it, expect, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createRouter, createMemoryHistory } from "vue-router";
import { defineComponent, h } from "vue";
import ModelDashboardHero from "@/components/dashboard/ModelDashboardHero.vue";
import { useUiModelStore, UI_MODELS } from "@/stores/uiModel";
import { navigationSections } from "@/data/navigation";

const Page = defineComponent({ render: () => h("div") });
const props = { cpu: 23, ram: 45, disk: 49, netDown: 120, health: 92, healthLabel: "Excellent" };

async function mountHero(model: string) {
  const pinia = createPinia();
  setActivePinia(pinia);
  useUiModelStore().setModel(model as never);
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: "/:p(.*)*", component: Page }] });
  router.push("/"); await router.isReady();
  const w = mount(ModelDashboardHero, { props, global: { plugins: [pinia, router] } });
  await flushPromises();
  return { w, router };
}

beforeEach(() => localStorage.clear());

describe("en-tête du tableau de bord par modèle", () => {
  it("rien en Classique (interface historique inchangée)", async () => {
    const { w } = await mountHero("classic");
    expect(w.find(".mdh").exists()).toBe(false);
  });

  it.each(UI_MODELS.filter(m => m.id !== "classic").map(m => m.id))("%s : rendu propre avec les mesures de la page", async (id) => {
    const { w } = await mountHero(id);
    const el = w.find(".mdh");
    expect(el.exists()).toBe(true);
    expect(el.classes().some(c => c.startsWith("mdh-"))).toBe(true);
    if (id !== "mission" && id !== "command-deck") expect(w.text()).toMatch(/23\s?%|92/);
  });

  it("Command Deck : chaque raccourci de section ouvre son premier outil", async () => {
    const { w, router } = await mountHero("command-deck");
    const keys = w.findAll(".mdh-deck__key");
    expect(keys).toHaveLength(navigationSections.length);
    await keys[4].trigger("click"); await flushPromises();
    expect(router.currentRoute.value.path).toBe(navigationSections[4].items[0].route);
  });

  it("Mission : démarrer une mission ouvre sa première étape", async () => {
    const { w, router } = await mountHero("mission");
    await w.findAll(".mdh-mission__tpl")[0].trigger("click"); await flushPromises();
    expect(router.currentRoute.value.path).toBe("/diagnostic");
  });
});
