import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { defineComponent, h, ref } from "vue";
import { useMenuKeys } from "@/composables/useMenuKeys";
import UiModelQuickSwitch from "@/components/shared/UiModelQuickSwitch.vue";
import { useUiModelStore } from "@/stores/uiModel";

beforeEach(() => { localStorage.clear(); setActivePinia(createPinia()); });
afterEach(() => { document.body.innerHTML = ""; });

describe("useMenuKeys", () => {
  const Menu = defineComponent({
    setup() {
      const root = ref<HTMLElement | null>(null);
      useMenuKeys(root, ".it", "vertical");
      return () => h("div", { ref: root }, ["A", "B", "C"].map(t => h("button", { class: "it" }, t)));
    },
  });

  it("flèches, Début et Fin déplacent le focus en boucle", async () => {
    const w = mount(Menu, { attachTo: document.body });
    await flushPromises();
    const items = w.findAll(".it");
    (items[0].element as HTMLElement).focus();
    await items[0].trigger("keydown", { key: "ArrowDown" });
    expect(document.activeElement?.textContent).toBe("B");
    await items[1].trigger("keydown", { key: "End" });
    expect(document.activeElement?.textContent).toBe("C");
    await items[2].trigger("keydown", { key: "ArrowDown" });
    expect(document.activeElement?.textContent).toBe("A");
    await items[0].trigger("keydown", { key: "ArrowUp" });
    expect(document.activeElement?.textContent).toBe("C");
    w.unmount();
  });
});

describe("changement rapide de modèle", () => {
  it("liste les 9 modèles et applique le choix", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const w = mount(UiModelQuickSwitch, { attachTo: document.body, global: { plugins: [pinia] } });
    await w.find(".uqs-btn").trigger("click");
    await flushPromises();
    const items = document.querySelectorAll<HTMLButtonElement>(".uqs-item");
    expect(items.length).toBe(9);
    document.querySelector<HTMLButtonElement>('.uqs-item[data-model="mission"]')!.click();
    await flushPromises();
    expect(useUiModelStore().model).toBe("mission");
    expect(document.documentElement.getAttribute("data-ui-model")).toBe("mission");
    expect(document.querySelector(".uqs-menu")).toBeNull();
    w.unmount();
  });
});
