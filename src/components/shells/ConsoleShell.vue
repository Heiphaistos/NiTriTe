<script setup lang="ts">
/**
 * Modèle 05 « Console » : arborescence monospace des outils (numérotés),
 * onglets des outils ouverts et ligne d'invite avec le chemin courant.
 * Ctrl+B masque l'arborescence.
 */
import { computed, inject, ref, type Ref } from "vue";
import { useRouter } from "vue-router";
import { X, Search, Settings, ChevronRight } from "lucide-vue-next";
import AppStatusBar from "@/components/layout/AppStatusBar.vue";
import PageSlot from "./PageSlot.vue";
import { useShellNav, useOpenTabs } from "@/composables/useShellNav";

const nav = useShellNav();
const tabs = useOpenTabs();
const router = useRouter();
const treeHidden = inject<Ref<boolean>>("sidebarCollapsed", ref(false));

function slug(text: string) {
  return text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase()
    .replace(/&/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}
const promptPath = computed(() => {
  const sec = nav.currentSection.value;
  return sec ? `~/${slug(nav.sectionShortLabel(sec.title))}/${nav.currentItem.value?.id ?? ""}` : `~${nav.route.path}`;
});
</script>

<template>
  <div class="sh-shell con">
    <nav v-show="!treeHidden" class="con-tree" aria-label="Arborescence des outils">
      <div class="con-tree__head"><span class="con-tree__n">N</span>NITRITE · EXPLORATEUR</div>
      <div class="con-tree__body sh-scroll">
        <div v-for="(section, si) in nav.sections" :key="section.title" class="con-group">
          <button
            class="con-group__head"
            :class="{ 'has-active': nav.sectionHasActive(section) }"
            :aria-expanded="!nav.isSectionCollapsed(section.title)"
            @click="nav.toggleSection(section.title)"
          >
            <ChevronRight :size="12" class="con-chev" :class="{ open: !nav.isSectionCollapsed(section.title) }" />
            <span class="con-group__label">{{ section.title }}</span>
            <span class="con-group__count">{{ section.items.length }}</span>
          </button>
          <template v-if="!nav.isSectionCollapsed(section.title) || nav.sectionHasActive(section)">
            <button
              v-for="(item, ii) in section.items"
              :key="item.id"
              class="con-leaf"
              :class="{ active: nav.isActive(item.route) }"
              :title="nav.navDescription(item.id)"
              @click="nav.navigate(item.route)"
            >
              <span class="con-leaf__label">{{ item.label }}</span>
              <span class="con-leaf__key">{{ si + 1 }}.{{ ii + 1 }}</span>
            </button>
          </template>
        </div>
      </div>
    </nav>

    <div class="sh-main">
      <div class="con-tabs" role="tablist" aria-label="Outils ouverts">
        <div
          v-for="path in tabs.tabs.value"
          :key="path"
          role="tab"
          :aria-selected="nav.isActive(path)"
          class="con-tab"
          :class="{ active: nav.isActive(path) }"
          @click="nav.navigate(path)"
        >
          <span>{{ tabs.labelOf(path) }}</span>
          <button class="con-tab__x" :aria-label="`Fermer ${tabs.labelOf(path)}`" @click.stop="tabs.close(path)">
            <X :size="11" />
          </button>
        </div>
      </div>
      <div class="con-prompt">
        <span class="con-prompt__host">nitrite@pc</span><span class="con-prompt__sep">:</span><span class="con-prompt__path">{{ promptPath }}</span><span class="con-prompt__dollar">$</span>
        <span class="con-prompt__title">{{ nav.pageTitle.value }}</span>
        <span class="con-prompt__spacer" />
        <button class="con-prompt__btn" title="Recherche globale (Ctrl+K)" @click="nav.openSearch">
          <Search :size="13" /> rechercher <kbd class="sh-kbd">Ctrl K</kbd>
        </button>
        <button class="con-prompt__btn" title="Paramètres" @click="router.push('/settings')">
          <Settings :size="13" />
        </button>
      </div>
      <PageSlot><slot /></PageSlot>
      <AppStatusBar />
    </div>
  </div>
</template>

<style scoped>
.con { display: flex; font-family: "JetBrains Mono", "Cascadia Code", Consolas, monospace; }
.con :deep(.app-content) { font-family: "Inter", system-ui, -apple-system, sans-serif; }

.con-tree {
  width: 260px; flex-shrink: 0; display: flex; flex-direction: column;
  background: var(--bg-primary); border-right: 1px solid var(--border); font-size: 12px;
}
.con-tree__head {
  display: flex; align-items: center; gap: 9px; height: 40px; padding: 0 12px; flex-shrink: 0;
  font-size: 11px; letter-spacing: 0.08em; color: var(--text-secondary); border-bottom: 1px solid var(--border);
}
.con-tree__n {
  width: 20px; height: 20px; display: grid; place-items: center; border-radius: 4px;
  background: var(--accent-primary); color: var(--bg-primary); font-weight: 700;
}
.con-tree__body { flex: 1; min-height: 0; padding: 6px 0 12px; }

.con-group__head, .con-leaf {
  width: 100%; display: flex; align-items: center; gap: 6px; border: none; background: transparent;
  font-family: inherit; text-align: left; cursor: pointer;
}
.con-group__head { padding: 5px 10px; color: var(--text-secondary); font-size: 11.5px; font-weight: 600; }
.con-group__head:hover { color: var(--text-primary); background: var(--surface-glass); }
.con-group__head.has-active { color: var(--accent-primary); }
.con-chev { flex-shrink: 0; transition: transform var(--transition-fast); }
.con-chev.open { transform: rotate(90deg); }
.con-group__label { flex: 1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; text-transform: uppercase; letter-spacing: 0.04em; }
.con-group__count { color: var(--text-muted); font-size: 10.5px; }

.con-leaf { padding: 4px 10px 4px 30px; color: var(--text-secondary); font-size: 12px; border-left: 2px solid transparent; }
.con-leaf::before { content: "├"; color: var(--border-strong); margin-left: -16px; margin-right: 4px; }
.con-leaf:hover { color: var(--text-primary); background: var(--surface-glass); }
.con-leaf.active { color: var(--accent-primary); background: var(--accent-subtle, var(--accent-muted)); border-left-color: var(--accent-primary); }
.con-leaf__label { flex: 1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.con-leaf__key { color: var(--text-muted); font-size: 10px; }

.con-tabs {
  display: flex; flex-shrink: 0; height: 36px; overflow-x: auto; scrollbar-width: none;
  background: var(--bg-primary); border-bottom: 1px solid var(--border);
}
.con-tab {
  display: flex; align-items: center; gap: 8px; padding: 0 8px 0 14px; flex-shrink: 0; cursor: pointer;
  font-size: 11.5px; color: var(--text-muted); border-right: 1px solid var(--border);
  border-top: 2px solid transparent; user-select: none;
}
.con-tab:hover { color: var(--text-primary); }
.con-tab.active { color: var(--text-primary); background: var(--bg-secondary); border-top-color: var(--accent-primary); }
.con-tab__x {
  width: 18px; height: 18px; display: grid; place-items: center; border-radius: 4px;
  border: none; background: transparent; color: inherit; cursor: pointer; opacity: 0.6;
}
.con-tab__x:hover { opacity: 1; background: var(--bg-tertiary); }

.con-prompt {
  display: flex; align-items: center; gap: 0; height: 34px; padding: 0 14px; flex-shrink: 0;
  font-size: 12px; background: var(--bg-secondary); border-bottom: 1px solid var(--border); white-space: nowrap; overflow: hidden;
}
.con-prompt__host { color: var(--success); }
.con-prompt__sep { color: var(--text-muted); }
.con-prompt__path { color: var(--info); }
.con-prompt__dollar { color: var(--text-muted); margin: 0 8px 0 4px; }
.con-prompt__title { color: var(--text-primary); font-weight: 700; }
.con-prompt__spacer { flex: 1; }
.con-prompt__btn {
  display: flex; align-items: center; gap: 6px; height: 24px; padding: 0 8px; margin-left: 6px;
  border: 1px solid var(--border); border-radius: 4px; background: transparent; cursor: pointer;
  color: var(--text-secondary); font-family: inherit; font-size: 11px;
}
.con-prompt__btn:hover { color: var(--accent-primary); border-color: var(--accent-primary); }

@media (max-width: 1280px) { .con-tree { width: 220px; } }
</style>
