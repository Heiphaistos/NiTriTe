<script setup lang="ts">
/**
 * Modèle 04 « Bento » : rail d'icônes des sections, panneau des outils de la
 * section choisie (avec description), et page posée dans une grande carte
 * arrondie. Ctrl+B masque le panneau d'outils.
 */
import { inject, ref, watch, computed, type Ref } from "vue";
import { useRouter } from "vue-router";
import { Search, Settings } from "lucide-vue-next";
import AppStatusBar from "@/components/layout/AppStatusBar.vue";
import logoUrl from "@/assets/nitrite-logo.jpg";
import { useShellNav } from "@/composables/useShellNav";

const nav = useShellNav();
const router = useRouter();
const panelHidden = inject<Ref<boolean>>("sidebarCollapsed", ref(false));

const selectedTitle = ref(nav.currentSection.value?.title ?? nav.sections[0].title);
watch(() => nav.currentSection.value?.title, (t) => { if (t) selectedTitle.value = t; });
const selected = computed(() => nav.sections.find(s => s.title === selectedTitle.value) ?? nav.sections[0]);

function pick(title: string) {
  selectedTitle.value = title;
  panelHidden.value = false;
}
</script>

<template>
  <div class="sh-shell bento">
    <nav class="bento-rail" aria-label="Sections">
      <img :src="logoUrl" class="sh-logo bento-logo" alt="NiTriTe" />
      <button
        v-for="section in nav.sections"
        :key="section.title"
        class="bento-rail__btn"
        :class="{ selected: section.title === selected.title && !panelHidden, 'has-active': nav.sectionHasActive(section) }"
        :title="section.title"
        :aria-label="section.title"
        @click="pick(section.title)"
      >
        <component :is="nav.getSectionIcon(section.title)" :size="19" />
      </button>
      <div class="bento-rail__spacer" />
      <button class="bento-rail__btn" title="Recherche globale (Ctrl+K)" aria-label="Rechercher" @click="nav.openSearch">
        <Search :size="18" />
      </button>
      <button class="bento-rail__btn" title="Paramètres" aria-label="Paramètres" @click="router.push('/settings')">
        <Settings :size="18" />
      </button>
    </nav>

    <section v-show="!panelHidden" class="bento-panel sh-scroll" :aria-label="`Sous-menu ${selected.title}`">
      <span class="bento-panel__title">{{ selected.title }}</span>
      <button
        v-for="item in selected.items"
        :key="item.id"
        class="sh-tool bento-tool"
        :class="{ active: nav.isActive(item.route) }"
        @click="nav.navigate(item.route)"
      >
        <span class="sh-tool__icon"><component :is="nav.getNavIcon(item.icon)" :size="17" /></span>
        <span class="sh-tool__text">
          <span class="sh-tool__label">{{ item.label }}</span>
          <span class="sh-tool__sub">{{ nav.navDescription(item.id) }}</span>
        </span>
      </button>
      <template v-if="nav.pinnedItems.value.length">
        <span class="bento-panel__title bento-panel__title--pins">Épinglés</span>
        <div class="bento-pins">
          <button
            v-for="item in nav.pinnedItems.value"
            :key="item.id"
            class="bento-pin"
            :class="{ active: nav.isActive(item.route) }"
            @click="nav.navigate(item.route)"
          >
            <component :is="nav.getNavIcon(item.icon)" :size="16" />
            <span>{{ item.label }}</span>
          </button>
        </div>
      </template>
    </section>

    <div class="sh-main bento-main">
      <header class="bento-head">
        <div class="bento-head__titles">
          <span class="bento-head__title">{{ nav.pageTitle.value }}</span>
          <span v-if="nav.currentItem.value" class="bento-head__sub">{{ nav.navDescription(nav.currentItem.value.id) }}</span>
        </div>
        <button class="sh-search bento-head__search" title="Recherche globale (Ctrl+K)" @click="nav.openSearch">
          <Search :size="15" />
          <span>Rechercher</span>
          <kbd class="sh-kbd">Ctrl K</kbd>
        </button>
      </header>
      <div class="bento-stage">
        <slot />
      </div>
      <AppStatusBar />
    </div>
  </div>
</template>

<style scoped>
.bento { display: flex; gap: 0; background: var(--bg-primary); }

.bento-rail {
  width: 72px; flex-shrink: 0; display: flex; flex-direction: column; align-items: center; gap: 6px;
  padding: 14px 0; background: var(--bg-primary);
}
.bento-logo { width: 40px; height: 40px; border-radius: 13px; margin-bottom: 10px; }
.bento-rail__btn {
  position: relative; width: 46px; height: 46px; flex-shrink: 0;
  display: flex; align-items: center; justify-content: center;
  border: 1px solid transparent; border-radius: 15px; background: transparent; cursor: pointer;
  color: var(--text-muted); transition: all var(--transition-fast);
}
.bento-rail__btn:hover { background: var(--bg-secondary); color: var(--text-primary); }
.bento-rail__btn.has-active::after {
  content: ""; position: absolute; left: -12px; top: 12px; bottom: 12px; width: 3px; border-radius: 3px;
  background: var(--accent-primary);
}
.bento-rail__btn.selected { background: var(--accent-primary); color: var(--bg-primary); box-shadow: var(--accent-glow-sm, none); }
.bento-rail__spacer { flex: 1; }

.bento-panel {
  width: 250px; flex-shrink: 0; margin: 12px 0 12px; padding: 16px 10px;
  display: flex; flex-direction: column; gap: 4px;
  background: var(--bg-secondary); border: 1px solid var(--border); border-radius: 22px;
}
.bento-panel__title { font-size: 17px; font-weight: 800; padding: 2px 8px 10px; letter-spacing: -0.01em; }
.bento-panel__title--pins { font-size: 11px; letter-spacing: 0.12em; text-transform: uppercase; color: var(--text-muted); padding-top: 16px; font-weight: 700; }
.bento-tool { border-radius: 14px; padding: 10px; }
.bento-tool .sh-tool__icon { width: 36px; height: 36px; border-radius: 12px; }
.bento-pins { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
.bento-pin {
  display: flex; flex-direction: column; align-items: flex-start; gap: 8px; padding: 10px;
  border-radius: 14px; border: 1px solid var(--border); background: var(--bg-primary); cursor: pointer;
  color: var(--text-secondary); font-size: 11.5px; font-weight: 600; text-align: left;
}
.bento-pin:hover, .bento-pin.active { color: var(--text-primary); border-color: var(--accent-primary); }
.bento-pin :deep(svg) { color: var(--accent-primary); }

.bento-main { padding: 12px 12px 0; }
.bento-head { display: flex; align-items: center; gap: 16px; padding: 4px 8px 12px; }
.bento-head__titles { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.bento-head__title { font-size: 22px; font-weight: 800; letter-spacing: -0.02em; }
.bento-head__sub { font-size: 12.5px; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.bento-head__search { width: 240px; border-radius: 12px; }
.bento-stage {
  flex: 1; min-height: 0; display: flex; flex-direction: column;
  border-radius: 22px 22px 0 0; border: 1px solid var(--border); border-bottom: none;
  background: var(--bg-secondary); overflow: hidden;
}
.bento-main :deep(.status-bar) { margin: 0 -12px; }
</style>
