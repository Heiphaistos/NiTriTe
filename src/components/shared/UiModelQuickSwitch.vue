<script setup lang="ts">
/** Changement rapide de modèle d'interface depuis la barre d'état. */
import { ref } from "vue";
import { onClickOutside, onKeyStroke } from "@vueuse/core";
import { LayoutTemplate, Check } from "lucide-vue-next";
import { UI_MODELS, useUiModelStore, type UiModelId } from "@/stores/uiModel";
import { useMenuKeys, focusFirst } from "@/composables/useMenuKeys";

const store = useUiModelStore();
const open = ref(false);
const root = ref<HTMLElement | null>(null);
const btn = ref<HTMLElement | null>(null);
const menu = ref<HTMLElement | null>(null);
// Menu téléporté dans <body> : certaines coques (Verre & dock) rognent leur contenu.
const pos = ref({ right: 0, bottom: 0 });

function toggle() {
  open.value = !open.value;
  if (!open.value) return;
  const r = btn.value?.getBoundingClientRect();
  if (r) pos.value = { right: Math.max(8, window.innerWidth - r.right), bottom: window.innerHeight - r.top + 8 };
  focusFirst(() => menu.value, ".uqs-item.active");
}
function pick(id: UiModelId) {
  store.setModel(id);
  open.value = false;
}
onClickOutside(menu, () => { open.value = false; }, { ignore: [btn] });
onKeyStroke("Escape", () => { open.value = false; });
useMenuKeys(menu, ".uqs-item", "vertical");
</script>

<template>
  <div ref="root" class="uqs">
    <button
      ref="btn"
      class="uqs-btn"
      :aria-expanded="open"
      aria-haspopup="menu"
      title="Changer de modèle d'interface (Ctrl+Maj+M)"
      @click="toggle"
    >
      <LayoutTemplate :size="11" />
      <span>{{ store.info.name }}</span>
    </button>
    <Teleport to="body">
    <div
      v-if="open"
      ref="menu"
      class="uqs-menu"
      role="menu"
      aria-label="Modèles d'interface"
      :style="{ right: `${pos.right}px`, bottom: `${pos.bottom}px` }"
    >
      <button
        v-for="m in UI_MODELS"
        :key="m.id"
        role="menuitemradio"
        :aria-checked="store.model === m.id"
        class="uqs-item"
        :class="{ active: store.model === m.id }"
        :data-model="m.id"
        @click="pick(m.id)"
      >
        <span class="uqs-name">{{ m.name }}</span>
        <Check v-if="store.model === m.id" :size="12" />
      </button>
    </div>
    </Teleport>
  </div>
</template>

<style scoped>
.uqs { position: relative; }
.uqs-btn {
  display: flex; align-items: center; gap: 5px; height: 20px; padding: 0 7px;
  border: 1px solid var(--border); border-radius: 5px; background: transparent; cursor: pointer;
  color: var(--text-muted); font-family: inherit; font-size: 10.5px;
}
.uqs-btn:hover, .uqs-btn[aria-expanded="true"] { color: var(--accent-primary); border-color: var(--accent-primary); }
.uqs-menu {
  position: fixed; z-index: 1000; min-width: 190px; padding: 5px;
  display: flex; flex-direction: column; gap: 1px;
  background: var(--bg-secondary); border: 1px solid var(--border); border-radius: var(--radius-md); box-shadow: var(--shadow-xl);
}
.uqs-item {
  display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 6px 9px;
  border: none; border-radius: 6px; background: transparent; cursor: pointer; text-align: left;
  color: var(--text-secondary); font-family: inherit; font-size: 12px;
}
.uqs-item:hover, .uqs-item:focus-visible { background: var(--bg-tertiary); color: var(--text-primary); outline: none; }
.uqs-item.active { color: var(--accent-primary); font-weight: 600; }
</style>
