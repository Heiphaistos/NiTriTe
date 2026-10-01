<script setup lang="ts">
/**
 * Modèle 10 « Mission » : bandeau des outils ouverts en onglets (fermables,
 * « + » ouvre la recherche), barre des sections avec menus déroulants, puis
 * la page. Pensé pour enchaîner plusieurs outils pendant une intervention.
 */
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { onClickOutside, onKeyStroke } from "@vueuse/core";
import { ChevronDown, Plus, Settings, X } from "lucide-vue-next";
import AppStatusBar from "@/components/layout/AppStatusBar.vue";
import PageSlot from "./PageSlot.vue";
import { useShellNav, useOpenTabs } from "@/composables/useShellNav";

const nav = useShellNav();
const tabs = useOpenTabs();
const router = useRouter();

const openTitle = ref<string | null>(null);
const openSection = computed(() => nav.sections.find(s => s.title === openTitle.value) ?? null);
const barRef = ref<HTMLElement | null>(null);
const menuLeft = ref(0);

function toggle(title: string, e: MouseEvent) {
  if (openTitle.value === title) { openTitle.value = null; return; }
  const btn = e.currentTarget as HTMLElement;
  const bar = barRef.value;
  menuLeft.value = bar ? Math.max(0, Math.min(btn.offsetLeft - bar.scrollLeft, bar.clientWidth - 340)) : 0;
  openTitle.value = title;
}
function go(route: string) { openTitle.value = null; nav.navigate(route); }
onClickOutside(barRef, () => { openTitle.value = null; });
onKeyStroke("Escape", () => { openTitle.value = null; });
watch(() => nav.route.path, () => { openTitle.value = null; });
</script>

<template>
  <div class="sh-shell mis">
    <div class="mis-tabs" role="tablist" aria-label="Outils ouverts">
      <span class="mis-logo">N</span>
      <div
        v-for="path in tabs.tabs.value"
        :key="path"
        role="tab"
        :aria-selected="nav.isActive(path)"
        class="mis-tab"
        :class="{ active: nav.isActive(path) }"
        @click="nav.navigate(path)"
      >
        <span v-if="nav.isActive(path)" class="mis-tab__dot" />
        <component :is="tabs.iconOf(path)" v-else :size="13" />
        <span class="mis-tab__label">{{ tabs.labelOf(path) }}</span>
        <button class="mis-tab__x" :aria-label="`Fermer ${tabs.labelOf(path)}`" @click.stop="tabs.close(path)"><X :size="11" /></button>
      </div>
      <button class="mis-tab-add" title="Ouvrir un outil (Ctrl+K)" aria-label="Ouvrir un outil" @click="nav.openSearch"><Plus :size="15" /></button>
      <div class="mis-tabs__spacer" />
      <button class="mis-tab-add" title="Paramètres" aria-label="Paramètres" @click="router.push('/settings')"><Settings :size="14" /></button>
    </div>

    <div ref="barRef" class="mis-barwrap">
      <nav class="mis-bar" aria-label="Sections">
        <button
          v-for="section in nav.sections"
          :key="section.title"
          class="mis-cat"
          :class="{ open: openTitle === section.title, 'has-active': nav.sectionHasActive(section) }"
          :aria-expanded="openTitle === section.title"
          @click="toggle(section.title, $event)"
        >
          {{ nav.sectionShortLabel(section.title) }}
          <ChevronDown :size="12" />
        </button>
      </nav>
      <Transition name="sh-fade">
        <section
          v-if="openSection"
          :key="openSection.title"
          class="mis-menu sh-pop"
          :style="{ left: `${menuLeft + 12}px` }"
          :aria-label="`Menu ${openSection.title}`"
        >
          <button
            v-for="item in openSection.items"
            :key="item.id"
            class="sh-tool"
            :class="{ active: nav.isActive(item.route) }"
            @click="go(item.route)"
          >
            <span class="sh-tool__icon"><component :is="nav.getNavIcon(item.icon)" :size="15" /></span>
            <span class="sh-tool__text">
              <span class="sh-tool__label">{{ item.label }}</span>
              <span class="sh-tool__sub">{{ nav.navDescription(item.id) }}</span>
            </span>
          </button>
        </section>
      </Transition>
    </div>

    <div class="mis-banner">
      <span class="mis-banner__kicker">{{ nav.currentSection.value ? nav.currentSection.value.title.toUpperCase() : "NITRITE" }}</span>
      <span class="mis-banner__title">{{ nav.pageTitle.value }}</span>
      <span v-if="nav.currentItem.value" class="mis-banner__sub">{{ nav.navDescription(nav.currentItem.value.id) }}</span>
    </div>

    <div class="sh-main">
      <PageSlot><slot /></PageSlot>
      <AppStatusBar />
    </div>
  </div>
</template>

<style scoped>
.mis { display: flex; flex-direction: column; }

.mis-tabs {
  height: 42px; flex-shrink: 0; display: flex; align-items: flex-end; gap: 2px; padding: 0 10px;
  background: var(--bg-primary); overflow-x: auto; scrollbar-width: none;
}
.mis-logo {
  align-self: center; width: 26px; height: 26px; margin-right: 8px; flex-shrink: 0; border-radius: 7px;
  display: grid; place-items: center; font-weight: 800; font-size: 13px; background: var(--accent-primary); color: var(--bg-primary);
}
.mis-tab {
  height: 34px; display: flex; align-items: center; gap: 8px; padding: 0 6px 0 12px; flex-shrink: 0; max-width: 200px;
  border-radius: 10px 10px 0 0; cursor: pointer; user-select: none;
  color: var(--text-muted); font-size: 12px; background: transparent;
}
.mis-tab:hover { color: var(--text-primary); background: var(--bg-tertiary); }
.mis-tab.active { color: var(--text-primary); background: var(--bg-secondary); font-weight: 600; }
.mis-tab__dot { width: 7px; height: 7px; border-radius: 50%; background: var(--accent-primary); box-shadow: 0 0 8px var(--accent-primary); flex-shrink: 0; }
.mis-tab__label { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.mis-tab__x { width: 18px; height: 18px; display: grid; place-items: center; border: none; border-radius: 5px; background: transparent; color: inherit; cursor: pointer; opacity: 0.55; flex-shrink: 0; }
.mis-tab__x:hover { opacity: 1; background: var(--bg-elevated); }
.mis-tab-add { align-self: center; width: 28px; height: 28px; display: grid; place-items: center; flex-shrink: 0; border: none; border-radius: 7px; background: transparent; color: var(--text-muted); cursor: pointer; }
.mis-tab-add:hover { background: var(--bg-tertiary); color: var(--text-primary); }
.mis-tabs__spacer { flex: 1; }

.mis-barwrap { position: relative; z-index: 60; flex-shrink: 0; }
.mis-bar {
  display: flex; gap: 2px; padding: 6px 12px; overflow-x: auto; scrollbar-width: none;
  background: var(--bg-secondary); border-bottom: 1px solid var(--border);
}
.mis-cat {
  display: flex; align-items: center; gap: 5px; padding: 6px 11px; flex-shrink: 0;
  border: none; border-radius: 7px; background: transparent; cursor: pointer; white-space: nowrap;
  color: var(--text-secondary); font-size: 12.5px; font-weight: 600;
}
.mis-cat:hover, .mis-cat.open { background: var(--bg-tertiary); color: var(--text-primary); }
.mis-cat.has-active { color: var(--accent-primary); }
.mis-menu { position: absolute; top: calc(100% + 4px); width: 340px; padding: 8px; display: flex; flex-direction: column; gap: 2px; }

.mis-banner {
  flex-shrink: 0; display: flex; align-items: baseline; gap: 14px; padding: 14px 24px 12px;
  border-bottom: 1px solid var(--border);
  background: linear-gradient(90deg, var(--accent-subtle, var(--accent-muted)), transparent 60%);
  white-space: nowrap; overflow: hidden;
}
.mis-banner__kicker { font-family: "JetBrains Mono", monospace; font-size: 10.5px; letter-spacing: 0.12em; color: var(--accent-primary); }
.mis-banner__title { font-size: 19px; font-weight: 800; }
.mis-banner__sub { font-size: 12.5px; color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; }
</style>
