<script setup lang="ts">
/**
 * Modèle 08 « Colonnes » : navigation façon explorateur — colonne des
 * sections, colonne des outils de la section (avec description), puis la
 * page. En-tête avec précédent / suivant et fil d'Ariane.
 * Ctrl+B masque la colonne des outils.
 */
import { computed, inject, ref, watch, type Ref } from "vue";
import { useRouter } from "vue-router";
import { ChevronLeft, ChevronRight, Search, Settings } from "lucide-vue-next";
import AppStatusBar from "@/components/layout/AppStatusBar.vue";
import { useShellNav } from "@/composables/useShellNav";

const nav = useShellNav();
const router = useRouter();
const toolsHidden = inject<Ref<boolean>>("sidebarCollapsed", ref(false));

const selectedTitle = ref(nav.currentSection.value?.title ?? nav.sections[0].title);
watch(() => nav.currentSection.value?.title, (t) => { if (t) selectedTitle.value = t; });
const selected = computed(() => nav.sections.find(s => s.title === selectedTitle.value) ?? nav.sections[0]);

function pick(title: string) {
  selectedTitle.value = title;
  toolsHidden.value = false;
}
</script>

<template>
  <div class="sh-shell col">
    <header class="col-top">
      <button class="sh-icon-btn" title="Précédent" aria-label="Précédent" @click="router.back()"><ChevronLeft :size="16" /></button>
      <button class="sh-icon-btn" title="Suivant" aria-label="Suivant" @click="router.forward()"><ChevronRight :size="16" /></button>
      <nav class="col-crumbs" aria-label="Fil d'Ariane">
        <span>NiTriTe</span>
        <template v-if="nav.currentSection.value">
          <span class="col-crumbs__sep">›</span>
          <button class="col-crumbs__link" @click="pick(nav.currentSection.value.title)">{{ nav.currentSection.value.title }}</button>
        </template>
        <span class="col-crumbs__sep">›</span>
        <span class="col-crumbs__page">{{ nav.pageTitle.value }}</span>
      </nav>
      <button class="sh-search col-search" title="Recherche globale (Ctrl+K)" @click="nav.openSearch">
        <Search :size="15" />
        <span>Rechercher partout</span>
        <kbd class="sh-kbd">Ctrl K</kbd>
      </button>
      <button class="sh-icon-btn" title="Paramètres" aria-label="Paramètres" @click="router.push('/settings')"><Settings :size="16" /></button>
    </header>

    <div class="col-body">
      <nav class="col-sections sh-scroll" aria-label="Sections">
        <button
          v-for="section in nav.sections"
          :key="section.title"
          class="col-sec"
          :class="{ selected: section.title === selected.title, 'has-active': nav.sectionHasActive(section) }"
          @click="pick(section.title)"
        >
          <component :is="nav.getSectionIcon(section.title)" :size="16" />
          <span>{{ section.title }}</span>
          <ChevronRight :size="13" class="col-sec__chev" />
        </button>
      </nav>

      <nav v-show="!toolsHidden" class="col-tools sh-scroll" :aria-label="`Outils ${selected.title}`">
        <span class="col-tools__title">{{ selected.title }}</span>
        <button
          v-for="item in selected.items"
          :key="item.id"
          class="col-tool"
          :class="{ active: nav.isActive(item.route) }"
          @click="nav.navigate(item.route)"
        >
          <span class="col-tool__label">
            <component :is="nav.getNavIcon(item.icon)" :size="14" />
            {{ item.label }}
          </span>
          <span class="col-tool__sub">{{ nav.navDescription(item.id) }}</span>
        </button>
      </nav>

      <div class="sh-main col-main">
        <slot />
      </div>
    </div>
    <AppStatusBar />
  </div>
</template>

<style scoped>
.col { display: flex; flex-direction: column; }

.col-top {
  height: 52px; flex-shrink: 0; display: flex; align-items: center; gap: 8px; padding: 0 14px;
  background: var(--bg-secondary); border-bottom: 1px solid var(--border);
}
.col-top .sh-icon-btn { width: 30px; height: 30px; }
.col-crumbs { flex: 1; min-width: 0; display: flex; align-items: center; gap: 8px; margin-left: 8px; font-size: 13px; color: var(--text-muted); white-space: nowrap; overflow: hidden; }
.col-crumbs__sep { color: var(--border-strong); }
.col-crumbs__link { border: none; background: transparent; color: var(--text-secondary); cursor: pointer; font-size: 13px; padding: 0; }
.col-crumbs__link:hover { color: var(--accent-primary); }
.col-crumbs__page { color: var(--text-primary); font-weight: 700; }
.col-search { width: 260px; height: 32px; }

.col-body { flex: 1; min-height: 0; display: flex; }

.col-sections {
  width: 210px; flex-shrink: 0; padding: 10px 8px; display: flex; flex-direction: column; gap: 2px;
  background: var(--bg-primary); border-right: 1px solid var(--border);
}
.col-sec {
  display: flex; align-items: center; gap: 10px; padding: 8px 10px; border: none; border-radius: 7px;
  background: transparent; color: var(--text-secondary); cursor: pointer; font-size: 12.5px; text-align: left;
}
.col-sec span { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.col-sec__chev { opacity: 0; }
.col-sec:hover { background: var(--bg-tertiary); color: var(--text-primary); }
.col-sec.has-active { color: var(--accent-primary); }
.col-sec.selected { background: var(--accent-primary); color: var(--bg-primary); font-weight: 600; }
.col-sec.selected .col-sec__chev { opacity: 1; }

.col-tools {
  width: 240px; flex-shrink: 0; padding: 12px 8px; display: flex; flex-direction: column; gap: 2px;
  background: var(--bg-secondary); border-right: 1px solid var(--border);
}
.col-tools__title { font-size: 10.5px; letter-spacing: 0.14em; text-transform: uppercase; color: var(--text-muted); padding: 0 10px 8px; }
.col-tool {
  display: flex; flex-direction: column; align-items: flex-start; gap: 2px; padding: 8px 10px;
  border: none; border-radius: 7px; background: transparent; cursor: pointer; text-align: left;
}
.col-tool:hover { background: var(--bg-tertiary); }
.col-tool.active { background: var(--accent-muted); box-shadow: inset 2px 0 0 var(--accent-primary); }
.col-tool__label { display: flex; align-items: center; gap: 8px; color: var(--text-primary); font-size: 12.5px; font-weight: 600; }
.col-tool__label :deep(svg) { color: var(--text-muted); }
.col-tool.active .col-tool__label :deep(svg) { color: var(--accent-primary); }
.col-tool__sub { font-size: 11px; color: var(--text-muted); padding-left: 22px; }

.col-main { background: var(--bg-primary); }
</style>
