<script setup lang="ts">
/**
 * Modèle 06 « Orbital » : barre supérieure avec les outils de la section
 * courante en pastilles rondes, et navigation circulaire (sections en orbite,
 * outils au centre) ouverte par le bouton « Orbite ».
 */
import { computed, inject, ref, watch, type Ref } from "vue";
import { useRouter } from "vue-router";
import { onKeyStroke } from "@vueuse/core";
import { Orbit, Search, Settings, X } from "lucide-vue-next";
import AppStatusBar from "@/components/layout/AppStatusBar.vue";
import PageSlot from "./PageSlot.vue";
import { useMenuKeys, focusFirst } from "@/composables/useMenuKeys";
import { useShellNav } from "@/composables/useShellNav";

const nav = useShellNav();
const router = useRouter();

const orbitOpen = ref(false);
const selectedTitle = ref(nav.currentSection.value?.title ?? nav.sections[0].title);
const selected = computed(() => nav.sections.find(s => s.title === selectedTitle.value) ?? nav.sections[0]);

const stage = ref<HTMLElement | null>(null);
const pills = ref<HTMLElement | null>(null);
useMenuKeys(stage, ".orb-node, .orb-core__item", "both");
useMenuKeys(pills, ".orb-pill", "horizontal");

function openOrbit() {
  selectedTitle.value = nav.currentSection.value?.title ?? selectedTitle.value;
  orbitOpen.value = true;
  focusFirst(() => stage.value, ".orb-node.selected");
}
// Ctrl+B (replier la navigation) ouvre / ferme l'orbite dans ce modèle.
const collapsed = inject<Ref<boolean>>("sidebarCollapsed", ref(false));
watch(collapsed, () => { if (orbitOpen.value) orbitOpen.value = false; else openOrbit(); });
function go(route: string) {
  orbitOpen.value = false;
  nav.navigate(route);
}
onKeyStroke("Escape", () => { orbitOpen.value = false; });
watch(() => nav.route.path, () => { orbitOpen.value = false; });

const RADIUS = 268;
const nodes = computed(() => nav.sections.map((section, i) => {
  const angle = (i / nav.sections.length) * Math.PI * 2 - Math.PI / 2;
  return { section, x: Math.cos(angle) * RADIUS, y: Math.sin(angle) * RADIUS };
}));
</script>

<template>
  <div class="sh-shell orb">
    <header class="orb-top">
      <span class="orb-brand">Ni<span>Tri</span>Te</span>
      <button class="orb-launch" title="Navigation orbitale" @click="openOrbit">
        <Orbit :size="16" />
        <span>Orbite</span>
      </button>
      <div ref="pills" class="orb-pills">
        <template v-if="nav.currentSection.value">
          <button
            v-for="item in nav.currentSection.value.items"
            :key="item.id"
            class="orb-pill"
            :class="{ active: nav.isActive(item.route) }"
            :title="nav.navDescription(item.id)"
            @click="nav.navigate(item.route)"
          >
            <component :is="nav.getNavIcon(item.icon)" :size="14" />
            <span>{{ item.label }}</span>
          </button>
        </template>
        <span v-else class="orb-pill orb-pill--title">{{ nav.pageTitle.value }}</span>
      </div>
      <button class="sh-icon-btn orb-round" title="Recherche globale (Ctrl+K)" @click="nav.openSearch"><Search :size="16" /></button>
      <button class="sh-icon-btn orb-round" title="Paramètres" @click="router.push('/settings')"><Settings :size="16" /></button>
    </header>

    <div class="sh-main">
      <PageSlot><slot /></PageSlot>
      <AppStatusBar />
    </div>

    <Transition name="orb-fade">
      <div v-if="orbitOpen" class="orb-overlay" role="dialog" aria-label="Navigation orbitale" @click.self="orbitOpen = false">
        <button class="sh-icon-btn orb-close" title="Fermer (Échap)" @click="orbitOpen = false"><X :size="18" /></button>
        <div ref="stage" class="orb-stage">
          <svg class="orb-rings" viewBox="-330 -330 660 660" aria-hidden="true">
            <circle r="318" class="ring ring--outer" />
            <circle r="268" class="ring ring--dash" />
            <circle r="170" class="ring ring--inner" />
          </svg>
          <button
            v-for="n in nodes"
            :key="n.section.title"
            class="orb-node"
            :class="{ selected: n.section.title === selected.title, 'has-active': nav.sectionHasActive(n.section) }"
            :style="{ transform: `translate(${n.x}px, ${n.y}px)` }"
            :aria-label="n.section.title"
            @click="selectedTitle = n.section.title"
            @focus="selectedTitle = n.section.title"
            @mouseenter="selectedTitle = n.section.title"
          >
            <component :is="nav.getSectionIcon(n.section.title)" :size="20" />
            <span>{{ nav.sectionShortLabel(n.section.title) }}</span>
          </button>
          <div class="orb-core">
            <span class="orb-core__title">{{ selected.title }}</span>
            <button
              v-for="item in selected.items"
              :key="item.id"
              class="orb-core__item"
              :class="{ active: nav.isActive(item.route) }"
              @click="go(item.route)"
            >
              <component :is="nav.getNavIcon(item.icon)" :size="14" />
              {{ item.label }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.orb { display: flex; flex-direction: column; position: relative; }

.orb-top {
  height: 60px; flex-shrink: 0; display: flex; align-items: center; gap: 12px; padding: 0 18px;
  background: radial-gradient(ellipse at 20% -40%, var(--accent-muted), transparent 60%), var(--bg-secondary);
  border-bottom: 1px solid var(--border);
}
.orb-brand { font-weight: 800; font-size: 18px; letter-spacing: 0.02em; margin-right: 6px; }
.orb-brand span { color: var(--accent-primary); }
.orb-launch {
  display: flex; align-items: center; gap: 8px; height: 38px; padding: 0 16px 0 12px; flex-shrink: 0;
  border-radius: 999px; border: 1px solid color-mix(in srgb, var(--accent-primary) 50%, transparent);
  background: var(--accent-muted); color: var(--accent-hover); font-weight: 700; font-size: 12.5px; cursor: pointer;
  box-shadow: var(--accent-glow-sm, none);
}
.orb-launch:hover { background: var(--accent-primary); color: var(--bg-primary); }
.orb-pills { flex: 1; min-width: 0; display: flex; gap: 6px; overflow-x: auto; scrollbar-width: none; }
.orb-pill {
  display: flex; align-items: center; gap: 6px; height: 32px; padding: 0 13px; flex-shrink: 0;
  border-radius: 999px; border: 1px solid var(--border); background: transparent; cursor: pointer;
  color: var(--text-secondary); font-size: 12px; white-space: nowrap;
}
.orb-pill:hover { color: var(--text-primary); border-color: var(--border-hover); }
.orb-pill.active { color: var(--text-primary); border-color: var(--accent-primary); background: var(--accent-muted); }
.orb-pill--title { cursor: default; font-weight: 700; color: var(--text-primary); }
.orb-round { border-radius: 50%; }

.orb-overlay {
  position: fixed; inset: 0; z-index: 900; display: flex; align-items: center; justify-content: center;
  background: color-mix(in srgb, var(--bg-primary) 82%, transparent);
  backdrop-filter: blur(10px);
}
html[data-perf="light"] .orb-overlay { backdrop-filter: none; background: var(--bg-primary); }
.orb-close { position: absolute; top: 18px; right: 18px; border-radius: 50%; }
.orb-stage { position: relative; width: 660px; height: 660px; display: flex; align-items: center; justify-content: center; }
@media (max-height: 760px) { .orb-stage { transform: scale(0.82); } }
@media (max-height: 620px) { .orb-stage { transform: scale(0.68); } }
.orb-rings { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }
.ring { fill: none; }
.ring--outer { stroke: color-mix(in srgb, var(--accent-primary) 12%, transparent); stroke-width: 1; }
.ring--dash { stroke: color-mix(in srgb, var(--accent-primary) 30%, transparent); stroke-width: 1; stroke-dasharray: 4 8; }
.ring--inner { stroke: var(--border); stroke-width: 10; }

.orb-node {
  position: absolute; left: calc(50% - 42px); top: calc(50% - 42px); width: 84px; height: 84px;
  display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 5px;
  border-radius: 50%; border: 1px solid var(--border); background: var(--bg-secondary); cursor: pointer;
  color: var(--text-secondary); font-size: 10.5px; font-weight: 600; text-align: center; line-height: 1.15;
  transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast), box-shadow var(--transition-fast);
}
.orb-node span { max-width: 70px; }
.orb-node:hover { color: var(--text-primary); border-color: var(--border-hover); }
.orb-node.has-active { border-color: var(--accent-primary); color: var(--accent-hover); }
.orb-node.selected { background: var(--accent-primary); color: var(--bg-primary); border-color: var(--accent-primary); box-shadow: var(--accent-glow, none); }

.orb-core {
  position: relative; width: 320px; height: 320px; border-radius: 50%;
  display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 3px;
  background: radial-gradient(circle, var(--bg-secondary) 60%, transparent 72%);
}
.orb-core__title { font-size: 15px; font-weight: 800; margin-bottom: 8px; text-align: center; max-width: 220px; }
.orb-core__item {
  display: flex; align-items: center; gap: 8px; padding: 5px 12px; border-radius: 999px;
  border: 1px solid transparent; background: transparent; cursor: pointer; color: var(--text-secondary); font-size: 12.5px;
}
.orb-core__item:hover { color: var(--text-primary); background: var(--bg-tertiary); }
.orb-core__item.active { color: var(--accent-hover); border-color: color-mix(in srgb, var(--accent-primary) 40%, transparent); }

.orb-fade-enter-active, .orb-fade-leave-active { transition: opacity 180ms ease; }
.orb-fade-enter-from, .orb-fade-leave-to { opacity: 0; }
html[data-perf="light"] .orb-fade-enter-active, html[data-perf="light"] .orb-fade-leave-active { transition: none; }
</style>
