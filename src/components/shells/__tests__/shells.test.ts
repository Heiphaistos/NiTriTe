import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { mount, flushPromises, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createRouter, createMemoryHistory, type Router } from "vue-router";
import { defineComponent, h, ref } from "vue";
import { navigationSections } from "@/data/navigation";
import { navDescriptions } from "@/data/navDescriptions";
import { iconMap } from "@/data/navIcons";
import { shells } from "@/components/shells";
import { UI_MODELS, DEFAULT_UI_MODEL, useUiModelStore } from "@/stores/uiModel";
import routerSource from "@/router/index.ts?raw";

const Page = defineComponent({ render: () => h("div", { class: "fake-page" }, "page") });
const items = navigationSections.flatMap(s => s.items);

function makeRouter(): Router {
  return createRouter({
    history: createMemoryHistory(),
    routes: [
      ...items.map(i => ({ path: i.route, component: Page })),
      { path: "/:p(.*)*", component: Page },
    ],
  });
}

async function mountShell(id: keyof typeof shells, start = "/monitoring") {
  const router = makeRouter();
  router.push(start);
  await router.isReady();
  const searches: number[] = [];
  const wrapper = mount(shells[id], {
    attachTo: document.body,
    slots: { default: () => h("main", { class: "app-content" }, "contenu") },
    global: {
      plugins: [createPinia(), router],
      provide: {
        sidebarCollapsed: ref(false),
        toggleSidebar: () => {},
        openSearch: () => { searches.push(1); },
      },
    },
  });
  await flushPromises();
  return { wrapper, router, searches };
}

/** Révèle les outils d'une section et renvoie les boutons d'outil affichés. */
type Reveal = (w: VueWrapper, sectionIndex: number) => Promise<string>;
const click = async (w: VueWrapper, sel: string, i: number) => {
  const all = w.findAll(sel);
  await all[i].trigger("click");
  await flushPromises();
};
const reveal: Record<string, Reveal> = {
  "forge":        async () => ".forge-sec:nth-child(IDX) .forge-item",
  "console":      async () => ".con-group:nth-child(IDX) .con-leaf",
  "command-deck": async (w, i) => { await click(w, ".deck-cat", i); return ".deck-menu .sh-tool"; },
  "bento":        async (w, i) => { await click(w, ".bento-rail__btn", i); return ".bento-tool"; },
  "orbital":      async (w, i) => {
    if (!w.find(".orb-overlay").exists()) await click(w, ".orb-launch", 0);
    await click(w, ".orb-node", i); return ".orb-core__item";
  },
  "columns":      async (w, i) => { await click(w, ".col-sec", i); return ".col-tool"; },
  "glass-dock":   async (w, i) => { await click(w, ".glass-dock__btn", i); return ".glass-menu .sh-tool"; },
  "mission":      async (w, i) => { await click(w, ".mis-cat", i); return ".mis-menu .sh-tool"; },
};

beforeEach(() => {
  localStorage.clear();
  setActivePinia(createPinia());
});
afterEach(() => { document.body.innerHTML = ""; });

describe("store des modèles d'interface", () => {
  it("Forge est le modèle par défaut et Classique est conservé", () => {
    const store = useUiModelStore();
    store.load();
    expect(DEFAULT_UI_MODEL).toBe("forge");
    expect(store.model).toBe("forge");
    expect(UI_MODELS.map(m => m.id)).toContain("classic");
    expect(document.documentElement.getAttribute("data-ui-model")).toBe("forge");
  });

  it("mémorise le choix et ignore une valeur inconnue", () => {
    const store = useUiModelStore();
    store.setModel("bento");
    expect(localStorage.getItem("nitrite-ui-model")).toBe("bento");
    store.model = "forge";
    store.load();
    expect(store.model).toBe("bento");
    localStorage.setItem("nitrite-ui-model", "hud");
    store.load();
    expect(store.model).toBe("forge");
  });

  it("chaque modèle a une coque", () => {
    for (const m of UI_MODELS) expect(shells[m.id], m.id).toBeTruthy();
  });
});

describe("données de navigation partagées", () => {
  it("chaque outil a une icône connue et une description", () => {
    for (const i of items) {
      expect(iconMap[i.icon], i.icon).toBeTruthy();
      expect(navDescriptions[i.id], i.id).toBeTruthy();
    }
  });

  it("chaque entrée du menu correspond à une route déclarée", () => {
    const routes = new Set([...routerSource.matchAll(/path: "(\/[^"]*)"/g)].map(m => m[1]));
    for (const i of items) expect(routes.has(i.route), i.route).toBe(true);
  });
});

describe.each(Object.keys(reveal))("coque %s", (id) => {
  it("donne accès à chaque outil de chaque section, et chaque clic navigue vers sa route", async () => {
    const { wrapper, router } = await mountShell(id as keyof typeof shells);
    for (const [si, section] of navigationSections.entries()) {
      for (const [ii, item] of section.items.entries()) {
        const sel = (await reveal[id](wrapper, si)).replace("IDX", String(si + 1));
        const buttons = wrapper.findAll(sel);
        expect(buttons.map(b => b.text()), `${section.title}`).toHaveLength(section.items.length);
        expect(buttons[ii].text()).toContain(item.label);
        await buttons[ii].trigger("click");
        await flushPromises();
        expect(router.currentRoute.value.path, item.label).toBe(item.route);
      }
    }
    wrapper.unmount();
  });

  it("affiche la page dans son emplacement et ouvre la recherche", async () => {
    const { wrapper, searches } = await mountShell(id as keyof typeof shells);
    expect(wrapper.find(".app-content").exists()).toBe(true);
    // Emplacement où App.vue téléporte la zone de page (préservée entre modèles).
    expect(wrapper.find("[data-page-slot] .app-content").exists()).toBe(true);
    const searchBtn = wrapper.findAll("button").find(b =>
      /Ctrl\+K/.test(b.attributes("title") ?? "") || /Ouvrir un outil/.test(b.attributes("title") ?? ""));
    expect(searchBtn, "bouton de recherche").toBeTruthy();
    await searchBtn!.trigger("click");
    expect(searches.length).toBe(1);
    wrapper.unmount();
  });
});

describe("coque classique", () => {
  it("rend la barre latérale historique avec toutes les entrées", async () => {
    const { wrapper } = await mountShell("classic");
    const text = wrapper.text();
    for (const i of items) expect(text, i.label).toContain(i.label);
    expect(wrapper.find(".app-content").exists()).toBe(true);
    wrapper.unmount();
  });
});

describe("onglets ouverts (Console)", () => {
  it("ouvre un onglet par page visitée et la fermeture ramène au voisin", async () => {
    const { wrapper, router } = await mountShell("console", "/diagnostic");
    await router.push("/cleaner");
    await flushPromises();
    expect(wrapper.findAll(".con-tab").map(t => t.text())).toEqual(["Diagnostic", "Nettoyeur Avancé"]);
    await wrapper.findAll(".con-tab__x")[1].trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe("/diagnostic");
    expect(wrapper.findAll(".con-tab")).toHaveLength(1);
    wrapper.unmount();
  });
});
