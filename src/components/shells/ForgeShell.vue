<script setup lang="ts">
/**
 * Modèle 01 « Forge » (par défaut) : barre latérale riche — recherche, favoris
 * en pastilles, catégories dépliables avec compteur, carte d'état — et en-tête
 * avec fil d'Ariane, titre et description de l'outil.
 */
import { computed, inject, ref, watch, type Ref } from "vue";
import { useRouter } from "vue-router";
import { Search, Star, ChevronDown, Settings, PanelLeftClose, PanelLeftOpen } from "lucide-vue-next";
import AppStatusBar from "@/components/layout/AppStatusBar.vue";
import PageSlot from "./PageSlot.vue";
import logoUrl from "@/assets/nitrite-logo.jpg";
import { useShellNav } from "@/composables/useShellNav";
import { useAppStore } from "@/stores/app";

const nav = useShellNav();
const router = useRouter();
const appStore = useAppStore();
const appVersion = __APP_VERSION__;

const collapsed = inject<Ref<boolean>>("sidebarCollapsed", ref(false));
const toggleSidebar = inject<() => void>("toggleSidebar", () => {});

const perfLabel = computed(() => ({
  light: "Profil léger", balanced: "Profil équilibré", full: "Profil complet",
}[appStore.perfTier] ?? "Profil auto"));
const perfSub = computed(() => appStore.perfTier === "light"
  ? "Effets coupés pour la fluidité" : "Animations et effets actifs");

// La section de la page ouverte est toujours dépliée.
watch(() => nav.currentSection.value?.title, (title) => {
  if (title && nav.isSectionCollapsed(title)) nav.toggleSection(title);
}, { immediate: true });
</script>

<template>
  <div class="sh-shell forge" :class="{ 'is-collapsed': collapsed }">
    <aside class="forge-side">
      <div class="forge-brand">
        <img :src="logoUrl" class="sh-logo" alt="NiTriTe" />
        <div v-if="!collapsed" class="forge-brand__text">
          <span class="forge-brand__name">NiTriTe</span>
          <span class="forge-brand__ver">v{{ appVersion }}</span>
        </div>
        <button
          class="forge-collapse sh-icon-btn"
          :title="collapsed ? 'Déplier la barre (Ctrl+B)' : 'Replier la barre (Ctrl+B)'"
          @click="toggleSidebar"
        >
          <PanelLeftOpen v-if="collapsed" :size="15" />
          <PanelLeftClose v-else :size="15" />
        </button>
      </div>

      <button v-if="!collapsed" class="sh-search" title="Recherche globale (Ctrl+K)" @click="nav.openSearch">
        <Search :size="15" />
        <span>Rechercher un outil…</span>
        <kbd class="sh-kbd">Ctrl K</kbd>
      </button>
      <button v-else class="sh-icon-btn forge-search-mini" title="Recherche globale (Ctrl+K)" @click="nav.openSearch">
        <Search :size="16" />
      </button>

      <div v-if="!collapsed && nav.pinnedItems.value.length" class="forge-pins">
        <div class="forge-label">Épinglés</div>
        <div class="forge-pins__list">
          <button
            v-for="item in nav.pinnedItems.value"
            :key="item.id"
            class="forge-chip"
            :class="{ active: nav.isActive(item.route) }"
            @click="nav.navigate(item.route)"
          >{{ item.label }}</button>
        </div>
      </div>

      <nav class="forge-nav sh-scroll" aria-label="Catégories">
        <template v-if="!collapsed">
          <div v-for="section in nav.sections" :key="section.title" class="forge-sec">
            <button
              class="forge-sec__head"
              :class="{ 'has-active': nav.sectionHasActive(section) }"
              :aria-expanded="!nav.isSectionCollapsed(section.title)"
              @click="nav.toggleSection(section.title)"
            >
              <component :is="nav.getSectionIcon(section.title)" :size="17" class="forge-sec__icon" />
              <span class="forge-sec__title">{{ section.title }}</span>
              <span class="forge-sec__count">{{ section.items.length }}</span>
              <ChevronDown :size="14" class="forge-sec__chev" :class="{ closed: nav.isSectionCollapsed(section.title) }" />
            </button>
            <div v-show="!nav.isSectionCollapsed(section.title)" class="forge-sec__items">
              <div v-for="item in section.items" :key="item.id" class="forge-item-row">
                <button
                  class="forge-item"
                  :class="{ active: nav.isActive(item.route) }"
                  :title="nav.navDescription(item.id)"
                  @click="nav.navigate(item.route)"
                >
                  <component :is="nav.getNavIcon(item.icon)" :size="15" />
                  <span>{{ item.label }}</span>
                </button>
                <button
                  class="forge-pin"
                  :class="{ on: nav.isPinned(item.id) }"
                  :title="nav.isPinned(item.id) ? 'Retirer des favoris' : 'Épingler'"
                  @click.stop="nav.togglePin(item.id)"
                >
                  <Star :size="12" />
                </button>
              </div>
            </div>
          </div>
        </template>
        <template v-else>
          <button
            v-for="item in nav.allItems"
            :key="item.id"
            class="forge-mini"
            :class="{ active: nav.isActive(item.route) }"
            :title="item.label"
            :aria-label="item.label"
            @click="nav.navigate(item.route)"
          >
            <component :is="nav.getNavIcon(item.icon)" :size="17" />
          </button>
        </template>
      </nav>

      <div v-if="!collapsed" class="forge-state">
        <span class="forge-state__dot" :class="`tier-${appStore.perfTier}`" />
        <div class="forge-state__text">
          <span>{{ perfLabel }}</span>
          <small>{{ perfSub }}</small>
        </div>
        <button class="sh-icon-btn" title="Paramètres" @click="router.push('/settings')">
          <Settings :size="15" />
        </button>
      </div>
      <button v-else class="sh-icon-btn forge-search-mini" title="Paramètres" @click="router.push('/settings')">
        <Settings :size="16" />
      </button>
    </aside>

    <div class="sh-main">
      <header class="forge-head">
        <nav class="forge-head__crumb" aria-label="Fil d'Ariane">
          <template v-if="nav.currentSection.value">
            <component :is="nav.getSectionIcon(nav.currentSection.value.title)" :size="15" />
            <span>{{ nav.currentSection.value.title }}</span>
            <span class="forge-head__sep">/</span>
          </template>
          <strong>{{ nav.pageTitle.value }}</strong>
        </nav>
        <span v-if="nav.currentItem.value" class="forge-head__desc">{{ nav.navDescription(nav.currentItem.value.id) }}</span>
        <div class="forge-head__spacer" />
        <button
          v-if="nav.currentItem.value"
          class="sh-icon-btn"
          :class="{ 'is-on': nav.isPinned(nav.currentItem.value.id) }"
          :title="nav.isPinned(nav.currentItem.value.id) ? 'Retirer des favoris' : 'Épingler cette page'"
          @click="nav.togglePin(nav.currentItem.value.id)"
        >
          <Star :size="16" />
        </button>
        <button class="sh-search forge-head__search" title="Recherche globale (Ctrl+K)" @click="nav.openSearch">
          <Search :size="15" />
          <span>Rechercher…</span>
          <kbd class="sh-kbd">Ctrl K</kbd>
        </button>
      </header>
      <PageSlot><slot /></PageSlot>
      <AppStatusBar />
    </div>
  </div>
</template>

<style scoped>
.forge { display: flex; }

.forge-side {
  width: 272px; flex-shrink: 0; display: flex; flex-direction: column; gap: 12px;
  padding: 16px 12px 12px;
  background: var(--bg-secondary);
  border-right: 1px solid var(--border);
  transition: width var(--transition-normal);
}
.is-collapsed .forge-side { width: 68px; align-items: center; padding: 14px 8px; }

.forge-brand { display: flex; align-items: center; gap: 11px; padding: 0 4px 4px; width: 100%; }
.is-collapsed .forge-brand { flex-direction: column; padding: 0; }
.forge-brand__text { display: flex; flex-direction: column; flex: 1; min-width: 0; }
.forge-brand__name { font-weight: 800; font-size: 16px; letter-spacing: 0.02em; }
.forge-brand__ver { font-family: "JetBrains Mono", monospace; font-size: 10.5px; color: var(--text-muted); }
.forge-collapse { width: 30px; height: 30px; border-color: transparent; }

.forge-label {
  font-size: 10px; letter-spacing: 0.14em; text-transform: uppercase; color: var(--text-muted);
  padding: 2px 8px 6px;
}
.forge-pins__list { display: flex; flex-wrap: wrap; gap: 6px; padding: 0 4px; }
.forge-chip {
  font-size: 11.5px; padding: 4px 10px; border-radius: 999px; cursor: pointer;
  background: var(--bg-tertiary); color: var(--text-secondary); border: 1px solid var(--border);
  transition: all var(--transition-fast);
}
.forge-chip:hover, .forge-chip.active {
  background: var(--accent-muted); color: var(--accent-hover);
  border-color: color-mix(in srgb, var(--accent-primary) 35%, transparent);
}

.forge-nav { flex: 1; min-height: 0; display: flex; flex-direction: column; gap: 2px; margin: 0 -4px; padding: 0 4px; }
.is-collapsed .forge-nav { align-items: center; gap: 4px; }

.forge-sec__head {
  width: 100%; display: flex; align-items: center; gap: 10px; padding: 8px 10px;
  border: none; border-radius: 9px; background: transparent; cursor: pointer;
  color: var(--text-secondary); font-size: 13px; font-weight: 600; text-align: left;
  transition: background var(--transition-fast), color var(--transition-fast);
}
.forge-sec__head:hover { background: var(--bg-tertiary); color: var(--text-primary); }
.forge-sec__head.has-active { color: var(--text-primary); font-weight: 700; }
.forge-sec__icon { flex-shrink: 0; color: var(--text-muted); }
.forge-sec__head.has-active .forge-sec__icon { color: var(--accent-primary); }
.forge-sec__title { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.forge-sec__count { font-family: "JetBrains Mono", monospace; font-size: 10px; color: var(--text-muted); }
.forge-sec__chev { color: var(--text-muted); transition: transform var(--transition-fast); }
.forge-sec__chev.closed { transform: rotate(-90deg); }

.forge-sec__items {
  display: flex; flex-direction: column; gap: 1px;
  margin: 2px 0 6px 18px; padding-left: 10px; border-left: 1px solid var(--border);
}
.forge-item-row { position: relative; display: flex; align-items: center; }
.forge-item {
  flex: 1; min-width: 0; display: flex; align-items: center; gap: 9px; padding: 7px 28px 7px 10px;
  border: none; border-radius: 8px; background: transparent; cursor: pointer;
  color: var(--text-secondary); font-size: 12.5px; text-align: left;
  transition: background var(--transition-fast), color var(--transition-fast);
}
.forge-item span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.forge-item:hover { background: var(--bg-tertiary); color: var(--text-primary); }
.forge-item.active {
  color: var(--text-primary); font-weight: 600;
  background: linear-gradient(90deg, var(--accent-muted), transparent);
  box-shadow: inset 2px 0 0 var(--accent-primary);
}
.forge-item.active :deep(svg) { color: var(--accent-primary); }
.forge-pin {
  position: absolute; right: 4px; width: 22px; height: 22px; border-radius: 6px;
  display: flex; align-items: center; justify-content: center;
  border: none; background: transparent; color: var(--text-muted); cursor: pointer;
  opacity: 0; transition: opacity var(--transition-fast), color var(--transition-fast);
}
.forge-item-row:hover .forge-pin, .forge-pin:focus-visible, .forge-pin.on { opacity: 1; }
.forge-pin.on { color: var(--accent-primary); }
.forge-pin.on :deep(svg) { fill: currentColor; }

.forge-mini {
  width: 40px; height: 36px; flex-shrink: 0; display: flex; align-items: center; justify-content: center;
  border: none; border-radius: 9px; background: transparent; color: var(--text-muted); cursor: pointer;
}
.forge-mini:hover { background: var(--bg-tertiary); color: var(--text-primary); }
.forge-mini.active { background: var(--accent-muted); color: var(--accent-primary); }

.forge-state {
  display: flex; align-items: center; gap: 10px; padding: 10px 10px 10px 12px;
  border-radius: 12px; background: var(--bg-primary); border: 1px solid var(--border);
}
.forge-state__dot { width: 8px; height: 8px; border-radius: 50%; background: var(--success); box-shadow: 0 0 10px var(--success); flex-shrink: 0; }
.forge-state__dot.tier-balanced { background: var(--info); box-shadow: 0 0 10px var(--info); }
.forge-state__dot.tier-light { background: var(--warning); box-shadow: none; }
.forge-state__text { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.forge-state__text span { font-size: 12px; font-weight: 600; }
.forge-state__text small { font-size: 10.5px; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.forge-search-mini { width: 40px; height: 36px; }

.forge-head {
  min-height: 56px; flex-shrink: 0; display: flex; align-items: center; gap: 14px;
  padding: 0 24px; border-bottom: 1px solid var(--border);
  background: linear-gradient(180deg, var(--bg-secondary), var(--bg-primary));
}
.forge-head__crumb { display: flex; align-items: center; gap: 8px; font-size: 13px; color: var(--text-muted); white-space: nowrap; min-width: 0; }
.forge-head__crumb :deep(svg) { color: var(--accent-primary); flex-shrink: 0; }
.forge-head__crumb strong { color: var(--text-primary); font-weight: 700; overflow: hidden; text-overflow: ellipsis; }
.forge-head__sep { color: var(--border-strong); }
.forge-head__desc {
  font-size: 12px; color: var(--text-secondary); padding: 5px 11px; border-radius: 999px;
  border: 1px solid var(--border); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0;
}
.forge-head__spacer { flex: 1; }
.forge-head__search { width: 230px; }

@media (max-width: 1100px) { .forge-head__desc { display: none; } .forge-head__search { width: auto; } }

@media (max-width: 1280px) { .forge-side { width: 240px; } }
</style>
