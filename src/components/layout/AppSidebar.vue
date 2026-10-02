<script setup lang="ts">
import { ref, reactive, watch, computed, inject, type Component } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useLocalStorage } from "@vueuse/core";
import { navigationSections } from "@/data/navigation";
import logoUrl from "@/assets/nitrite-logo.jpg";
import type { SidebarPosition, SidebarMode, SidebarWidth } from "@/stores/layoutStore";
import {
  LayoutDashboard, Stethoscope, Activity, Zap, LayoutGrid, Wrench,
  Download, Package, HardDrive, RefreshCw, Cpu, Scan, Save, Shield, ShieldCheck,
  Wifi, Terminal, FileCode, Bot, BookOpen, FileText, ScrollText,
  BarChart3, Settings, ChevronLeft, ChevronRight, ChevronDown, Search,
  Palette, Trash2, Gauge, Server, Globe, Bug, TerminalSquare,
  Bluetooth, Sparkles, Copy, Database, Star, StarOff, User,
  Thermometer, PieChart, Files, FileSearch, Hash, Container, Radio, Network, Code2,
  Rocket, ClipboardList, ListChecks,
} from "lucide-vue-next";

const props = withDefaults(defineProps<{
  collapsed: boolean;
  position?: SidebarPosition;
  width?: SidebarWidth;
  mode?: SidebarMode;
}>(), {
  position: "left",
  width: "normal",
  mode: "icons-text",
});

const emit = defineEmits<{ toggle: [] }>();
const appVersion = __APP_VERSION__;

const route = useRoute();
const router = useRouter();

// Sections rétractables — persistance localStorage
const collapsedSections = reactive<Record<string, boolean>>({});

function loadSectionStates() {
  try {
    const saved = localStorage.getItem("nitrite-sections");
    if (saved) Object.assign(collapsedSections, JSON.parse(saved));
  } catch {}
}

function saveSectionStates() {
  localStorage.setItem("nitrite-sections", JSON.stringify(collapsedSections));
}

function toggleSection(title: string) {
  collapsedSections[title] = !collapsedSections[title];
  saveSectionStates();
}

function isSectionCollapsed(title: string) {
  return collapsedSections[title] === true;
}

function expandActiveSection() {
  for (const section of navigationSections) {
    for (const item of section.items) {
      if (item.route === route.path) {
        collapsedSections[section.title] = false;
        saveSectionStates();
        return;
      }
    }
  }
}

loadSectionStates();
watch(() => route.path, expandActiveSection);

// Mode icônes seulement (icons-only ou sidebar collapsed)
const iconsOnly = computed(() => props.collapsed || props.mode === "icons-only");

const sidebarWidthPx = computed(() => {
  if (props.collapsed) return 64;
  if (props.width === "compact") return 48;
  if (props.width === "large") return 290;
  return 240;
});

const iconMap: Record<string, Component> = {
  "layout-dashboard": LayoutDashboard,
  stethoscope: Stethoscope, activity: Activity, zap: Zap,
  "layout-grid": LayoutGrid, wrench: Wrench, download: Download,
  package: Package, "hard-drive": HardDrive, "refresh-cw": RefreshCw,
  cpu: Cpu, scan: Scan, save: Save, shield: Shield, wifi: Wifi,
  terminal: Terminal, "file-code": FileCode, bot: Bot,
  "book-open": BookOpen, "file-text": FileText, "scroll-text": ScrollText,
  "bar-chart-3": BarChart3, settings: Settings, palette: Palette,
  "trash-2": Trash2, gauge: Gauge, server: Server, globe: Globe,
  bug: Bug, "terminal-square": TerminalSquare, bluetooth: Bluetooth,
  sparkles: Sparkles, copy: Copy, database: Database, "shield-check": ShieldCheck,
  user: User,
  thermometer: Thermometer, "pie-chart": PieChart, files: Files,
  "file-search": FileSearch, hash: Hash, container: Container,
  radio: Radio, network: Network, "code-2": Code2,
  rocket: Rocket, "clipboard-list": ClipboardList, "list-checks": ListChecks,
};

function getIcon(name: string) { return iconMap[name] ?? LayoutDashboard; }
function isActive(itemRoute: string) { return route.path === itemRoute; }
function sectionHasActive(section: typeof navigationSections[0]) {
  return section.items.some(item => item.route === route.path);
}
function navigate(itemRoute: string) { router.push(itemRoute); }

const openSearch = inject<() => void>("openSearch", () => {});

// Favoris
const pinnedIds = useLocalStorage<string[]>("nitrite-pinned", []);
function isPinned(id: string) { return pinnedIds.value.includes(id); }
function togglePin(id: string, e: Event) {
  e.stopPropagation();
  if (isPinned(id)) {
    pinnedIds.value = pinnedIds.value.filter(x => x !== id);
  } else {
    pinnedIds.value = [...pinnedIds.value, id];
  }
}

const allNavItems = computed(() => navigationSections.flatMap(s => s.items));
const pinnedItems = computed(() =>
  pinnedIds.value
    .map(id => allNavItems.value.find(item => item.id === id))
    .filter(Boolean) as typeof allNavItems.value
);
</script>

<template>
  <aside
    class="sidebar"
    :class="{
      collapsed: props.collapsed,
      'icons-only': iconsOnly,
      'pos-right': props.position === 'right',
    }"
    :style="{ width: `${sidebarWidthPx}px` }"
  >
    <!-- Header -->
    <div class="sidebar-header">
      <div class="logo-area">
        <img :src="logoUrl" class="logo-img" alt="NiTriTe" />
        <transition name="fade">
          <div v-if="!iconsOnly" class="logo-text">
            <span class="logo-title">Nitrite</span>
            <span class="logo-version">v{{ appVersion }}</span>
          </div>
        </transition>
      </div>
      <button class="collapse-btn" @click="emit('toggle')" :title="props.collapsed ? 'Ouvrir' : 'Replier'">
        <template v-if="props.position === 'right'">
          <ChevronRight v-if="!props.collapsed" :size="16" />
          <ChevronLeft v-else :size="16" />
        </template>
        <template v-else>
          <ChevronLeft v-if="!props.collapsed" :size="16" />
          <ChevronRight v-else :size="16" />
        </template>
      </button>
    </div>

    <!-- Search -->
    <div class="sidebar-search-wrap" :class="{ collapsed: iconsOnly }">
      <button class="sidebar-search-btn" @click="openSearch" :title="iconsOnly ? 'Rechercher (Ctrl+K)' : undefined">
        <Search :size="13" class="sidebar-search-icon" />
        <transition name="fade">
          <span v-if="!iconsOnly" class="sidebar-search-placeholder">Rechercher…</span>
        </transition>
        <transition name="fade">
          <kbd v-if="!iconsOnly" class="sidebar-search-kbd">Ctrl K</kbd>
        </transition>
      </button>
    </div>

    <!-- Navigation -->
    <nav class="sidebar-nav">
      <!-- Favoris -->
      <div v-if="pinnedItems.length > 0 && !iconsOnly" class="nav-section">
        <div class="section-header pinned-header">
          <span class="section-title-text">Favoris</span>
          <Star :size="10" style="color:var(--warning)" />
        </div>
        <div class="section-items">
          <button
            v-for="item in pinnedItems"
            :key="`pin-${item.id}`"
            class="nav-item"
            :class="{ active: isActive(item.route) }"
            @click="navigate(item.route)"
          >
            <component :is="getIcon(item.icon)" :size="18" class="nav-icon" />
            <span class="nav-label">{{ item.label }}</span>
            <span class="pin-btn pinned" role="button" tabindex="0"
              @click="togglePin(item.id, $event)"
              @keydown.enter.prevent="togglePin(item.id, $event)"
              title="Retirer des favoris"
            ><StarOff :size="11" /></span>
          </button>
        </div>
        <div class="section-sep"></div>
      </div>

      <!-- Sections -->
      <div v-for="section in navigationSections" :key="section.title" class="nav-section">
        <button
          v-if="!iconsOnly"
          class="section-header"
          :class="{ 'has-active': sectionHasActive(section) }"
          @click="toggleSection(section.title)"
        >
          <span class="section-title-text">{{ section.title }}</span>
          <ChevronDown
            :size="12" class="section-chevron"
            :class="{ rotated: isSectionCollapsed(section.title) }"
          />
        </button>
        <div v-else class="section-divider"></div>

        <transition name="section-collapse">
          <div v-show="!isSectionCollapsed(section.title) || iconsOnly" class="section-items">
            <button
              v-for="item in section.items"
              :key="item.id"
              class="nav-item"
              :class="{ active: isActive(item.route) }"
              @click="navigate(item.route)"
              :title="iconsOnly ? item.label : undefined"
            >
              <component :is="getIcon(item.icon)" :size="18" class="nav-icon" />
              <transition name="fade">
                <span v-if="!iconsOnly" class="nav-label">{{ item.label }}</span>
              </transition>
              <span v-if="item.badge && !iconsOnly" class="nav-badge">{{ item.badge }}</span>
              <span
                v-if="!iconsOnly"
                class="pin-btn"
                :class="{ pinned: isPinned(item.id) }"
                role="button"
                tabindex="0"
                @click="togglePin(item.id, $event)"
                @keydown.enter.prevent="togglePin(item.id, $event)"
                :title="isPinned(item.id) ? 'Retirer des favoris' : 'Épingler'"
              ><Star :size="10" /></span>
            </button>
          </div>
        </transition>
      </div>
    </nav>

    <!-- Footer -->
    <div class="sidebar-footer">
      <button
        class="nav-item"
        :class="{ active: isActive('/settings') }"
        @click="navigate('/settings')"
        :title="iconsOnly ? 'Paramètres' : undefined"
      >
        <Settings :size="18" class="nav-icon" />
        <transition name="fade">
          <span v-if="!iconsOnly" class="nav-label">Paramètres</span>
        </transition>
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  position: fixed;
  top: 0; bottom: 0;
  left: 0;
  background: linear-gradient(180deg, var(--bg-secondary) 0%, var(--bg-primary) 85%);
  border-right: 1px solid var(--border);
  display: flex; flex-direction: column;
  z-index: 100;
  transition: width var(--transition-slow);
  overflow: hidden;
  box-shadow: inset -1px 0 0 var(--border), 1px 0 20px rgba(0,0,0,0.3);
}

.sidebar.pos-right {
  left: auto; right: 0;
  border-right: none; border-left: 1px solid var(--border);
  box-shadow: inset 1px 0 0 var(--border), -1px 0 20px rgba(0,0,0,0.3);
}

/* Header */
.sidebar-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 16px 12px; border-bottom: 1px solid var(--border); min-height: 64px;
}
.logo-area { display: flex; align-items: center; gap: 10px; overflow: hidden; }
.logo-img {
  width: 48px; height: 48px; border-radius: var(--radius-lg);
  object-fit: cover; flex-shrink: 0; display: block;
  box-shadow: 0 0 18px rgba(249, 115, 22, 0.45), 0 4px 12px rgba(0,0,0,0.5);
  transition: transform .2s; cursor: pointer;
}
.logo-img:hover { transform: scale(1.06); }
.logo-text { display: flex; flex-direction: column; white-space: nowrap; }
.logo-title {
  font-weight: 900; font-size: 19px;
  background: linear-gradient(135deg, var(--accent-primary), var(--accent-hover));
  -webkit-background-clip: text; -webkit-text-fill-color: transparent;
  background-clip: text; line-height: 1.2;
}
.logo-version { font-size: 11px; color: var(--text-secondary); font-family: "JetBrains Mono", monospace; }
.collapse-btn {
  width: 28px; height: 28px; border-radius: var(--radius-sm);
  border: none; background: transparent; color: var(--text-muted);
  cursor: pointer; display: flex; align-items: center; justify-content: center;
  flex-shrink: 0; transition: all var(--transition-fast);
}
.collapse-btn:hover { background: var(--bg-tertiary); color: var(--text-primary); }

/* Search */
.sidebar-search-wrap { padding: 8px 10px 4px; }
.sidebar-search-wrap.collapsed { padding: 8px 8px 4px; }
.sidebar-search-btn {
  display: flex; align-items: center; gap: 7px; width: 100%;
  padding: 6px 9px; background: var(--bg-tertiary); border: 1px solid var(--border);
  border-radius: var(--radius-md); cursor: pointer; font-family: inherit;
  transition: border-color var(--transition-fast), background var(--transition-fast);
  white-space: nowrap; overflow: hidden;
}
.sidebar-search-btn:hover { border-color: var(--accent-primary); background: var(--bg-secondary); }
.sidebar-search-icon { color: var(--text-muted); flex-shrink: 0; transition: color var(--transition-fast); }
.sidebar-search-btn:hover .sidebar-search-icon { color: var(--accent-primary); }
.sidebar-search-placeholder { flex: 1; font-size: 12px; color: var(--text-muted); text-align: left; }
.sidebar-search-kbd {
  margin-left: auto; flex-shrink: 0; font-size: 9px; font-family: inherit;
  color: var(--text-muted); background: var(--bg-primary); border: 1px solid var(--border);
  border-radius: 4px; padding: 1px 5px; letter-spacing: 0.03em;
}

/* Navigation */
.sidebar-nav { flex: 1; overflow-y: auto; overflow-x: hidden; padding: 8px; }
.nav-section { margin-bottom: 2px; }

.section-header {
  display: flex; align-items: center; justify-content: space-between;
  width: 100%; padding: 10px 8px 5px; border: none; background: transparent;
  cursor: pointer; transition: all var(--transition-fast); border-radius: var(--radius-sm);
}
.section-header:hover { background: var(--bg-tertiary); }
.section-header:hover .section-title-text { color: var(--text-secondary); }
.section-header.has-active .section-title-text { color: var(--accent-primary); }
.section-title-text {
  font-size: 10px; font-weight: 700; text-transform: uppercase;
  letter-spacing: 0.08em; color: var(--text-secondary); white-space: nowrap;
  transition: color var(--transition-fast);
}
.section-chevron { color: var(--text-muted); transition: transform var(--transition-normal); flex-shrink: 0; }
.section-chevron.rotated { transform: rotate(-90deg); }
.section-divider { height: 1px; background: var(--border); margin: 8px 8px; }
.section-items { overflow: hidden; }

/* Collapse transition */
.section-collapse-enter-active, .section-collapse-leave-active {
  transition: all 200ms ease; max-height: 500px; opacity: 1;
}
.section-collapse-enter-from, .section-collapse-leave-to { max-height: 0; opacity: 0; }

.nav-item {
  display: flex; align-items: center; gap: 10px;
  width: 100%; padding: 7px 10px; border: none; border-radius: var(--radius-md);
  background: transparent; color: var(--text-secondary); cursor: pointer;
  font-size: 13px; font-family: inherit; text-align: left;
  transition: all var(--transition-fast); position: relative; white-space: nowrap;
}
.nav-item:hover { background: var(--surface-glass-hover, rgba(255,255,255,0.05)); color: var(--text-primary); }
.nav-item.active {
  background: linear-gradient(90deg, rgba(249,115,22,.16) 0%, rgba(249,115,22,.03) 100%);
  color: var(--accent-primary); font-weight: 600;
}
.nav-item.active::before {
  content: ""; position: absolute; left: 0; top: 50%; transform: translateY(-50%);
  width: 3px; height: 20px; border-radius: 0 3px 3px 0;
  background: linear-gradient(180deg, var(--accent-primary), var(--accent-hover));
  box-shadow: 0 0 10px rgba(249,115,22,0.6), 0 0 4px rgba(249,115,22,0.4);
}
.pos-right .nav-item.active::before { left: auto; right: 0; border-radius: 3px 0 0 3px; }

.nav-icon { flex-shrink: 0; }
.nav-label { overflow: hidden; text-overflow: ellipsis; }
.nav-badge {
  margin-left: auto; background: var(--accent-primary); color: white;
  font-size: 10px; font-weight: 600; padding: 1px 6px; border-radius: 10px; min-width: 18px; text-align: center;
}

/* Icônes only — centrer les items */
.icons-only .nav-item { justify-content: center; padding: 8px; }
.icons-only .nav-item.active::before { left: 0; }

/* Pin */
.pin-btn {
  margin-left: auto; background: none; border: none; padding: 3px; border-radius: 4px;
  color: var(--text-muted); cursor: pointer; opacity: 0;
  transition: opacity .15s, color .15s; flex-shrink: 0; display: flex; align-items: center;
}
.nav-item:hover .pin-btn { opacity: 1; }
.pin-btn:hover { color: var(--warning); }
.pin-btn.pinned { opacity: 1; color: var(--warning); }
.pinned-header { cursor: default; pointer-events: none; }
.section-sep { height: 1px; background: var(--border); margin: 4px 8px 8px; }

/* Footer */
.sidebar-footer { border-top: 1px solid var(--border); padding: 8px; }

/* Transitions */
.fade-enter-active, .fade-leave-active { transition: opacity 150ms ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; }
</style>
