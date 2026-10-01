<script setup lang="ts">
/**
 * Modèle 02 « Command Deck » : palette de commandes en tête, barre de
 * catégories horizontale avec menus déroulants détaillés, et rangée des outils
 * de la section courante. Contenu sur toute la largeur.
 */
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { onClickOutside, onKeyStroke } from "@vueuse/core";
import { Search, Settings, ChevronDown } from "lucide-vue-next";
import AppStatusBar from "@/components/layout/AppStatusBar.vue";
import logoUrl from "@/assets/nitrite-logo.jpg";
import { useShellNav } from "@/composables/useShellNav";
import type { NavSection } from "@/data/navigation";

const nav = useShellNav();
const router = useRouter();

const openTitle = ref<string | null>(null);
const barRef = ref<HTMLElement | null>(null);
const openSection = computed(() => nav.sections.find(s => s.title === openTitle.value) ?? null);

function toggle(section: NavSection) {
  openTitle.value = openTitle.value === section.title ? null : section.title;
}
function go(route: string) {
  openTitle.value = null;
  nav.navigate(route);
}
onClickOutside(barRef, () => { openTitle.value = null; });
onKeyStroke("Escape", () => { openTitle.value = null; });
watch(() => nav.route.path, () => { openTitle.value = null; });
</script>

<template>
  <div class="sh-shell deck">
    <header class="deck-top">
      <div class="deck-brand">
        <img :src="logoUrl" class="sh-logo" alt="NiTriTe" />
        <span>NiTriTe</span>
      </div>
      <button class="deck-cmd" title="Palette de commandes (Ctrl+K)" @click="nav.openSearch">
        <Search :size="16" />
        <span>Tapez une commande : « nettoyer », « drivers », « réseau »…</span>
        <kbd class="sh-kbd">Ctrl K</kbd>
      </button>
      <button class="sh-icon-btn" title="Paramètres" @click="router.push('/settings')">
        <Settings :size="16" />
      </button>
    </header>

    <div ref="barRef" class="deck-barwrap">
      <nav class="deck-bar" aria-label="Sections">
        <button
          v-for="section in nav.sections"
          :key="section.title"
          class="deck-cat"
          :class="{ 'has-active': nav.sectionHasActive(section), open: openTitle === section.title }"
          :aria-expanded="openTitle === section.title"
          @click="toggle(section)"
        >
          <component :is="nav.getSectionIcon(section.title)" :size="15" />
          <span>{{ nav.sectionShortLabel(section.title) }}</span>
          <ChevronDown :size="12" class="deck-cat__chev" />
        </button>
      </nav>

      <Transition name="sh-fade">
        <section
          v-if="openSection"
          :key="openSection.title"
          class="deck-menu sh-pop"
          :aria-label="`Menu ${openSection.title}`"
        >
          <div class="deck-menu__head">
            <component :is="nav.getSectionIcon(openSection.title)" :size="20" />
            <div>
              <span class="deck-menu__title">{{ openSection.title }}</span>
              <span class="deck-menu__sub">{{ openSection.items.length }} outil{{ openSection.items.length > 1 ? "s" : "" }}</span>
            </div>
          </div>
          <div class="deck-menu__grid">
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
    </div>

    <div v-if="nav.currentSection.value" class="deck-strip">
      <span class="deck-strip__sec">{{ nav.currentSection.value.title }}</span>
      <button
        v-for="item in nav.currentSection.value.items"
        :key="item.id"
        class="deck-strip__item"
        :class="{ active: nav.isActive(item.route) }"
        @click="nav.navigate(item.route)"
      >
        <component :is="nav.getNavIcon(item.icon)" :size="13" />
        {{ item.label }}
      </button>
    </div>

    <div class="sh-main">
      <slot />
      <AppStatusBar />
    </div>
  </div>
</template>

<style scoped>
.deck { display: flex; flex-direction: column; }

.deck-top {
  height: 58px; flex-shrink: 0; display: flex; align-items: center; gap: 16px; padding: 0 20px;
  background: var(--bg-secondary); border-bottom: 1px solid var(--border);
}
.deck-brand { display: flex; align-items: center; gap: 10px; font-weight: 800; font-size: 16px; letter-spacing: 0.02em; }
.deck-brand .sh-logo { width: 30px; height: 30px; border-radius: 8px; }
.deck-cmd {
  flex: 1; max-width: 640px; margin: 0 auto; height: 38px;
  display: flex; align-items: center; gap: 10px; padding: 0 12px;
  border-radius: 10px; border: 1px solid var(--border); cursor: pointer;
  background: var(--bg-primary); color: var(--text-muted); font-size: 13px;
  box-shadow: inset 0 0 0 1px transparent;
  transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
}
.deck-cmd:hover { border-color: var(--accent-primary); box-shadow: 0 0 0 3px var(--accent-muted); }
.deck-cmd span { flex: 1; text-align: left; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

.deck-barwrap { position: relative; z-index: 60; flex-shrink: 0; }
.deck-bar {
  display: flex; gap: 2px; padding: 6px 14px; overflow-x: auto; scrollbar-width: none;
  background: var(--bg-primary); border-bottom: 1px solid var(--border);
}
.deck-cat {
  display: flex; align-items: center; gap: 7px; padding: 8px 12px; flex-shrink: 0;
  border: 1px solid transparent; border-radius: 9px; background: transparent; cursor: pointer;
  color: var(--text-secondary); font-size: 12.5px; font-weight: 600; white-space: nowrap;
  transition: background var(--transition-fast), color var(--transition-fast);
}
.deck-cat:hover { background: var(--bg-tertiary); color: var(--text-primary); }
.deck-cat.has-active { color: var(--accent-primary); background: var(--accent-muted); }
.deck-cat.open { color: var(--text-primary); background: var(--bg-tertiary); border-color: var(--border); }
.deck-cat__chev { opacity: 0.6; transition: transform var(--transition-fast); }
.deck-cat.open .deck-cat__chev { transform: rotate(180deg); }

.deck-menu {
  position: absolute; top: calc(100% + 6px); left: 14px; right: 14px; max-width: 980px;
  padding: 16px; display: flex; gap: 20px;
}
.deck-menu__head { width: 200px; flex-shrink: 0; display: flex; gap: 12px; align-items: flex-start; color: var(--accent-primary); }
.deck-menu__head div { display: flex; flex-direction: column; gap: 3px; }
.deck-menu__title { font-size: 15px; font-weight: 800; color: var(--text-primary); }
.deck-menu__sub { font-size: 11.5px; color: var(--text-muted); }
.deck-menu__grid { flex: 1; display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 4px; }

.deck-strip {
  flex-shrink: 0; display: flex; align-items: center; gap: 4px; padding: 6px 20px; overflow-x: auto; scrollbar-width: none;
  border-bottom: 1px solid var(--border); background: var(--bg-secondary);
}
.deck-strip__sec {
  font-size: 10px; letter-spacing: 0.14em; text-transform: uppercase; color: var(--text-muted);
  margin-right: 8px; white-space: nowrap;
}
.deck-strip__item {
  display: flex; align-items: center; gap: 6px; padding: 5px 10px; flex-shrink: 0;
  border: none; border-radius: 7px; background: transparent; cursor: pointer;
  color: var(--text-secondary); font-size: 12px; white-space: nowrap;
}
.deck-strip__item:hover { color: var(--text-primary); background: var(--bg-tertiary); }
.deck-strip__item.active { color: var(--text-primary); background: var(--bg-tertiary); box-shadow: inset 0 -2px 0 var(--accent-primary); }
</style>
