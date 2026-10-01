import { computed, inject, reactive, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useLocalStorage } from "@vueuse/core";
import { navigationSections, type NavItem, type NavSection } from "@/data/navigation";
import { navDescription } from "@/data/navDescriptions";
import { getNavIcon, getSectionIcon, sectionShortLabel } from "@/data/navIcons";

/**
 * Logique de navigation partagée par toutes les coques (modèles d'interface) :
 * mêmes entrées (navigation.ts), mêmes favoris (`nitrite-pinned`), mêmes
 * sections repliées (`nitrite-sections`). Aucune coque ne définit ses propres
 * routes : toutes les pages restent accessibles dans tous les modèles.
 */

const allItems: NavItem[] = navigationSections.flatMap(s => s.items);

export function findSectionOf(path: string): NavSection | undefined {
  return navigationSections.find(s => s.items.some(i => i.route === path));
}

// État partagé entre coques (module) : sections repliées.
const collapsedSections = reactive<Record<string, boolean>>({});
try {
  const saved = localStorage.getItem("nitrite-sections");
  if (saved) Object.assign(collapsedSections, JSON.parse(saved));
} catch { /* stockage indisponible ou corrompu */ }

function saveSections() {
  try { localStorage.setItem("nitrite-sections", JSON.stringify(collapsedSections)); } catch { /* ignore */ }
}

export const MAX_OPEN_TABS = 8;

export function useShellNav() {
  const route = useRoute();
  const router = useRouter();

  const currentItem = computed(() => allItems.find(i => i.route === route.path) ?? null);
  const currentSection = computed(() => findSectionOf(route.path) ?? null);
  const pageTitle = computed(() =>
    currentItem.value?.label ?? (route.meta?.title as string | undefined) ?? "NiTriTe");

  function isActive(itemRoute: string) { return route.path === itemRoute; }
  function sectionHasActive(section: NavSection) { return section.items.some(i => i.route === route.path); }
  function navigate(itemRoute: string) {
    if (route.path !== itemRoute) router.push(itemRoute);
  }

  // ── Sections repliées (Forge / Classique) ──
  function isSectionCollapsed(title: string) { return collapsedSections[title] === true; }
  function toggleSection(title: string) {
    collapsedSections[title] = !collapsedSections[title];
    saveSections();
  }

  // ── Favoris ──
  const pinnedIds = useLocalStorage<string[]>("nitrite-pinned", []);
  const pinnedItems = computed(() =>
    pinnedIds.value.map(id => allItems.find(i => i.id === id)).filter(Boolean) as NavItem[]);
  function isPinned(id: string) { return pinnedIds.value.includes(id); }
  function togglePin(id: string) {
    pinnedIds.value = isPinned(id) ? pinnedIds.value.filter(x => x !== id) : [...pinnedIds.value, id];
  }

  const openSearch = inject<() => void>("openSearch", () => {});

  return {
    sections: navigationSections,
    allItems,
    route,
    currentItem, currentSection, pageTitle,
    isActive, sectionHasActive, navigate,
    isSectionCollapsed, toggleSection,
    pinnedIds, pinnedItems, isPinned, togglePin,
    openSearch,
    getNavIcon, getSectionIcon, sectionShortLabel, navDescription,
  };
}

/**
 * Onglets des outils ouverts (modèles Console et Mission) : chaque page visitée
 * ouvre un onglet ; fermer l'onglet actif ramène sur son voisin.
 */
export function useOpenTabs() {
  const route = useRoute();
  const router = useRouter();
  const tabs = useLocalStorage<string[]>("nitrite-open-tabs", []);

  function labelOf(path: string) {
    return allItems.find(i => i.route === path)?.label ?? path;
  }
  function iconOf(path: string) {
    return getNavIcon(allItems.find(i => i.route === path)?.icon ?? "");
  }

  function track(path: string) {
    if (!allItems.some(i => i.route === path)) return;
    if (tabs.value.includes(path)) return;
    const next = [...tabs.value, path];
    // Trop d'onglets : on retire le plus ancien qui n'est pas la page courante.
    while (next.length > MAX_OPEN_TABS) next.splice(next.findIndex(p => p !== path), 1);
    tabs.value = next;
  }

  watch(() => route.path, track, { immediate: true });
  // Nettoie un stockage obsolète (routes disparues).
  tabs.value = tabs.value.filter(p => allItems.some(i => i.route === p));

  function close(path: string) {
    const idx = tabs.value.indexOf(path);
    if (idx < 0) return;
    const next = tabs.value.filter(p => p !== path);
    tabs.value = next;
    if (route.path === path) router.push(next[Math.min(idx, next.length - 1)] ?? "/");
  }

  return { tabs, labelOf, iconOf, close };
}
