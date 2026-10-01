<script setup lang="ts">
/**
 * Sélecteur du modèle d'interface, avec un aperçu schématique de chaque
 * disposition. Le changement est immédiat et mémorisé ; les pages et leurs
 * fonctions sont les mêmes dans tous les modèles.
 */
import { Check } from "lucide-vue-next";
import { UI_MODELS, useUiModelStore, type UiModelId } from "@/stores/uiModel";

const store = useUiModelStore();
function choose(id: UiModelId) { store.setModel(id); }
</script>

<template>
  <div class="ump" role="radiogroup" aria-label="Modèle d'interface">
    <button
      v-for="m in UI_MODELS"
      :key="m.id"
      class="ump-card"
      :class="{ active: store.model === m.id }"
      role="radio"
      :aria-checked="store.model === m.id"
      :data-model="m.id"
      @click="choose(m.id)"
    >
      <span class="ump-prev" :class="`ump-prev--${m.layout}`" aria-hidden="true">
        <span class="p-a" /><span class="p-b" /><span class="p-c" /><span class="p-d" />
      </span>
      <span class="ump-text">
        <span class="ump-name">
          {{ m.name }}
          <Check v-if="store.model === m.id" :size="13" class="ump-check" />
        </span>
        <span class="ump-tag">{{ m.tagline }}</span>
      </span>
    </button>
  </div>
</template>

<style scoped>
.ump { display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 10px; }
.ump-card {
  display: flex; flex-direction: column; gap: 10px; padding: 10px; text-align: left; cursor: pointer;
  border: 1px solid var(--border); border-radius: var(--radius-lg); background: var(--bg-tertiary);
  color: var(--text-primary); font-family: inherit;
  transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
}
.ump-card:hover { border-color: var(--border-hover); }
.ump-card.active { border-color: var(--accent-primary); box-shadow: 0 0 0 1px var(--accent-primary); }
.ump-text { display: flex; flex-direction: column; gap: 3px; }
.ump-name { display: flex; align-items: center; gap: 6px; font-weight: 700; font-size: 13px; }
.ump-check { color: var(--accent-primary); }
.ump-tag { font-size: 11px; color: var(--text-muted); line-height: 1.35; }

/* ── Aperçus schématiques (4 blocs : a = navigation, b = secondaire, c = page, d = détail) ── */
.ump-prev {
  position: relative; height: 74px; border-radius: 8px; overflow: hidden;
  background: var(--bg-primary); border: 1px solid var(--border);
}
.ump-prev > span { position: absolute; border-radius: 3px; background: var(--bg-elevated); }
.ump-prev .p-c { background: var(--bg-secondary); border: 1px solid var(--border); }
.ump-card.active .ump-prev .p-a { background: var(--accent-primary); }

.ump-prev--sidebar .p-a { left: 0; top: 0; bottom: 0; width: 26%; border-radius: 0; }
.ump-prev--sidebar .p-b { left: 30%; right: 4%; top: 6%; height: 12%; }
.ump-prev--sidebar .p-c { left: 30%; right: 4%; top: 24%; bottom: 8%; }
.ump-prev--sidebar .p-d { display: none; }

.ump-prev--topbar .p-a { left: 0; right: 0; top: 0; height: 14%; border-radius: 0; }
.ump-prev--topbar .p-b { left: 4%; right: 4%; top: 19%; height: 10%; opacity: 0.7; }
.ump-prev--topbar .p-c { left: 4%; right: 4%; top: 34%; bottom: 8%; }
.ump-prev--topbar .p-d { left: 18%; width: 40%; top: 29%; height: 34%; background: var(--bg-tertiary); border: 1px solid var(--border-hover); }

.ump-prev--rail .p-a { left: 3%; top: 6%; bottom: 6%; width: 8%; border-radius: 6px; }
.ump-prev--rail .p-b { left: 14%; top: 6%; bottom: 6%; width: 22%; border-radius: 8px; opacity: 0.7; }
.ump-prev--rail .p-c { left: 40%; right: 3%; top: 20%; bottom: 6%; border-radius: 10px; }
.ump-prev--rail .p-d { left: 40%; width: 30%; top: 6%; height: 9%; }

.ump-prev--tree .p-a { left: 0; top: 0; bottom: 0; width: 24%; border-radius: 0; opacity: 0.8; }
.ump-prev--tree .p-b { left: 26%; right: 0; top: 0; height: 13%; border-radius: 0; }
.ump-prev--tree .p-c { left: 27%; right: 2%; top: 30%; bottom: 4%; border-radius: 2px; }
.ump-prev--tree .p-d { left: 27%; width: 50%; top: 17%; height: 8%; }

.ump-prev--orbit .p-a { left: 0; right: 0; top: 0; height: 14%; border-radius: 0; }
.ump-prev--orbit .p-b { left: 36%; width: 28%; top: 22%; height: 56%; border-radius: 50%; opacity: 0.6; }
.ump-prev--orbit .p-c { left: 4%; right: 4%; top: 22%; bottom: 6%; opacity: 0.6; }
.ump-prev--orbit .p-d { left: 44%; width: 12%; top: 38%; height: 24%; border-radius: 50%; background: var(--accent-primary); }

.ump-prev--columns .p-a { left: 0; top: 14%; bottom: 0; width: 18%; border-radius: 0; }
.ump-prev--columns .p-b { left: 19%; top: 14%; bottom: 0; width: 21%; border-radius: 0; opacity: 0.7; }
.ump-prev--columns .p-c { left: 42%; right: 3%; top: 20%; bottom: 6%; }
.ump-prev--columns .p-d { left: 0; right: 0; top: 0; height: 11%; border-radius: 0; }

.ump-prev--dock .p-a { left: 22%; right: 22%; bottom: 5%; height: 15%; border-radius: 8px; }
.ump-prev--dock .p-b { left: 4%; width: 30%; top: 5%; height: 10%; opacity: 0.7; }
.ump-prev--dock .p-c { left: 4%; right: 4%; top: 20%; bottom: 26%; border-radius: 8px; }
.ump-prev--dock .p-d { right: -10%; top: -20%; width: 50%; height: 80%; border-radius: 50%; background: var(--accent-muted); }

.ump-prev--tabs .p-a { left: 0; right: 0; top: 0; height: 12%; border-radius: 0; }
.ump-prev--tabs .p-b { left: 0; right: 0; top: 13%; height: 10%; border-radius: 0; opacity: 0.6; }
.ump-prev--tabs .p-c { left: 4%; right: 4%; top: 30%; bottom: 6%; border-top: 2px solid var(--accent-primary); }
.ump-prev--tabs .p-d { left: 4%; width: 22%; top: 2%; height: 10%; background: var(--bg-secondary); }
</style>
