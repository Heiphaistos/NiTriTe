<script setup lang="ts">
/**
 * Modèle 09 « Verre & dock » : halos colorés du thème en fond, page posée sur
 * un panneau de verre dépoli, et dock des sections en bas ; chaque section
 * ouvre au-dessus du dock le panneau de ses outils.
 */
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { onClickOutside, onKeyStroke } from "@vueuse/core";
import { Search, Settings } from "lucide-vue-next";
import AppStatusBar from "@/components/layout/AppStatusBar.vue";
import PageSlot from "./PageSlot.vue";
import logoUrl from "@/assets/nitrite-logo.jpg";
import { useMenuKeys, focusFirst } from "@/composables/useMenuKeys";
import { useShellNav } from "@/composables/useShellNav";

const nav = useShellNav();
const router = useRouter();

const openTitle = ref<string | null>(null);
const openSection = computed(() => nav.sections.find(s => s.title === openTitle.value) ?? null);
const dockRef = ref<HTMLElement | null>(null);

const dockBar = ref<HTMLElement | null>(null);
const menuRef = ref<HTMLElement | null>(null);
useMenuKeys(dockBar, ".glass-dock__btn", "horizontal");
useMenuKeys(menuRef, ".sh-tool", "both");
function toggle(title: string) {
  openTitle.value = openTitle.value === title ? null : title;
  if (openTitle.value) focusFirst(() => menuRef.value, ".sh-tool");
}
function go(route: string) { openTitle.value = null; nav.navigate(route); }
onClickOutside(dockRef, () => { openTitle.value = null; });
onKeyStroke("Escape", () => { openTitle.value = null; });
watch(() => nav.route.path, () => { openTitle.value = null; });
</script>

<template>
  <div class="sh-shell glass">
    <div class="glass-aura glass-aura--a" aria-hidden="true" />
    <div class="glass-aura glass-aura--b" aria-hidden="true" />

    <header class="glass-top">
      <img :src="logoUrl" class="sh-logo" alt="NiTriTe" />
      <div class="glass-top__titles">
        <span class="glass-top__sec">{{ nav.currentSection.value?.title ?? "NiTriTe" }}</span>
        <span class="glass-top__title">{{ nav.pageTitle.value }}</span>
      </div>
      <div class="glass-top__spacer" />
      <button class="sh-search glass-chip" title="Recherche globale (Ctrl+K)" @click="nav.openSearch">
        <Search :size="15" />
        <span>Rechercher</span>
        <kbd class="sh-kbd">Ctrl K</kbd>
      </button>
      <button class="sh-icon-btn glass-chip" title="Paramètres" aria-label="Paramètres" @click="router.push('/settings')"><Settings :size="16" /></button>
    </header>

    <div class="sh-main glass-pane">
      <PageSlot><slot /></PageSlot>
      <AppStatusBar />
    </div>

    <div ref="dockRef" class="glass-dockwrap">
      <Transition name="sh-fade">
        <section v-if="openSection" ref="menuRef" :key="openSection.title" class="glass-menu" :aria-label="`Outils ${openSection.title}`">
          <span class="glass-menu__title">{{ openSection.title }}</span>
          <div class="glass-menu__grid">
            <button
              v-for="item in openSection.items"
              :key="item.id"
              class="sh-tool"
              :class="{ active: nav.isActive(item.route) }"
              @click="go(item.route)"
            >
              <span class="sh-tool__icon"><component :is="nav.getNavIcon(item.icon)" :size="16" /></span>
              <span class="sh-tool__text">
                <span class="sh-tool__label">{{ item.label }}</span>
                <span class="sh-tool__sub">{{ nav.navDescription(item.id) }}</span>
              </span>
            </button>
          </div>
        </section>
      </Transition>
      <nav ref="dockBar" class="glass-dock" aria-label="Sections">
        <button
          v-for="section in nav.sections"
          :key="section.title"
          class="glass-dock__btn"
          :class="{ open: openTitle === section.title, 'has-active': nav.sectionHasActive(section) }"
          :title="section.title"
          :aria-label="section.title"
          :aria-expanded="openTitle === section.title"
          @click="toggle(section.title)"
        >
          <component :is="nav.getSectionIcon(section.title)" :size="21" />
          <span class="glass-dock__label">{{ nav.sectionShortLabel(section.title) }}</span>
        </button>
      </nav>
    </div>
  </div>
</template>

<style scoped>
.glass {
  position: relative; display: flex; flex-direction: column; overflow: hidden;
  background: var(--bg-primary); padding: 0 14px 92px;
}
.glass-aura { position: absolute; border-radius: 50%; filter: blur(90px); pointer-events: none; opacity: 0.55; }
.glass-aura--a { width: 520px; height: 520px; left: -140px; top: -160px; background: var(--accent-primary); opacity: 0.22; }
.glass-aura--b { width: 460px; height: 460px; right: -120px; bottom: -80px; background: var(--info); opacity: 0.16; }
html[data-perf="light"] .glass-aura { display: none; }

.glass-top { position: relative; z-index: 2; height: 62px; flex-shrink: 0; display: flex; align-items: center; gap: 12px; padding: 0 6px; }
.glass-top__titles { display: flex; flex-direction: column; min-width: 0; }
.glass-top__title { font-size: 13.5px; font-weight: 700; }
.glass-top__sec { font-size: 10.5px; letter-spacing: 0.1em; text-transform: uppercase; color: var(--text-muted); }
.glass-top__spacer { flex: 1; }
.glass-chip {
  background: color-mix(in srgb, var(--bg-secondary) 60%, transparent);
  backdrop-filter: blur(14px); border-color: color-mix(in srgb, var(--text-primary) 10%, transparent);
  border-radius: 12px;
}
.sh-search.glass-chip { width: 220px; }

.glass-pane {
  position: relative; z-index: 1; border-radius: 22px; overflow: hidden;
  background: color-mix(in srgb, var(--bg-secondary) 72%, transparent);
  border: 1px solid color-mix(in srgb, var(--text-primary) 9%, transparent);
  backdrop-filter: blur(22px) saturate(1.3);
  box-shadow: var(--shadow-lg);
}
html[data-perf="light"] .glass-pane, html[data-perf="light"] .glass-chip, html[data-perf="light"] .glass-dock, html[data-perf="light"] .glass-menu {
  backdrop-filter: none; background: var(--bg-secondary);
}

.glass-dockwrap { position: absolute; z-index: 70; left: 50%; bottom: 14px; transform: translateX(-50%); display: flex; flex-direction: column; align-items: center; gap: 10px; }
.glass-dock {
  display: flex; gap: 4px; padding: 8px; border-radius: 22px;
  background: color-mix(in srgb, var(--bg-secondary) 65%, transparent);
  border: 1px solid color-mix(in srgb, var(--text-primary) 12%, transparent);
  backdrop-filter: blur(24px) saturate(1.4); box-shadow: var(--shadow-xl);
}
.glass-dock__btn {
  position: relative; width: 62px; height: 56px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 4px;
  border: none; border-radius: 15px; background: transparent; cursor: pointer; color: var(--text-secondary);
  transition: background var(--transition-fast), color var(--transition-fast), transform var(--transition-fast);
}
.glass-dock__btn:hover { background: color-mix(in srgb, var(--text-primary) 8%, transparent); color: var(--text-primary); transform: translateY(-2px); }
html[data-perf="light"] .glass-dock__btn:hover { transform: none; }
.glass-dock__btn.open { background: var(--accent-primary); color: var(--bg-primary); }
.glass-dock__btn.has-active::after {
  content: ""; position: absolute; bottom: 3px; width: 5px; height: 5px; border-radius: 50%; background: var(--accent-primary);
}
.glass-dock__btn.open.has-active::after { background: var(--bg-primary); }
.glass-dock__label { font-size: 9.5px; font-weight: 600; white-space: nowrap; max-width: 58px; overflow: hidden; text-overflow: ellipsis; }

.glass-menu {
  width: min(720px, calc(100vw - 40px)); padding: 14px; border-radius: 20px;
  background: color-mix(in srgb, var(--bg-secondary) 82%, transparent);
  border: 1px solid color-mix(in srgb, var(--text-primary) 12%, transparent);
  backdrop-filter: blur(24px); box-shadow: var(--shadow-xl);
}
.glass-menu__title { display: block; font-weight: 800; font-size: 14px; padding: 0 6px 10px; }
.glass-menu__grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 4px; }

@media (max-height: 760px) {
  .glass { padding-bottom: 80px; }
  .glass-dock__btn { width: 56px; height: 48px; }
  .glass-dock__label { display: none; }
}
</style>
