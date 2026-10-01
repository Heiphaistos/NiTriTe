import { defineStore } from "pinia";
import { ref, computed } from "vue";

/**
 * Modèle d'interface : la « coque » autour des pages (navigation, en-tête) et
 * l'habillage des composants. Les pages, leurs boutons et leurs fonctions sont
 * identiques dans tous les modèles ; seul l'aspect change. Les couleurs
 * viennent toujours du thème actif.
 */
export type UiModelId =
  | "forge" | "classic" | "command-deck" | "bento" | "console"
  | "orbital" | "columns" | "glass-dock" | "mission";

export interface UiModelInfo {
  id: UiModelId;
  name: string;
  tagline: string;
  /** Disposition résumée pour l'aperçu du sélecteur. */
  layout: "sidebar" | "topbar" | "rail" | "tree" | "orbit" | "columns" | "dock" | "tabs";
}

export const UI_MODELS: UiModelInfo[] = [
  { id: "forge",        name: "Forge",         tagline: "Barre latérale riche, favoris et catégories dépliables (par défaut)", layout: "sidebar" },
  { id: "classic",      name: "Classique",     tagline: "L'interface historique de NiTriTe, à l'identique",                   layout: "sidebar" },
  { id: "command-deck", name: "Command Deck",  tagline: "Barre de catégories en haut, menus déroulants et palette de commandes", layout: "topbar" },
  { id: "bento",        name: "Bento",         tagline: "Rail d'icônes et panneau d'outils détaillé, cartes arrondies",         layout: "rail" },
  { id: "console",      name: "Console",       tagline: "Arborescence monospace et onglets d'outils ouverts",                  layout: "tree" },
  { id: "orbital",      name: "Orbital",       tagline: "Navigation circulaire autour de la santé du PC",                      layout: "orbit" },
  { id: "columns",      name: "Colonnes",      tagline: "Navigation en colonnes : sections, outils, contenu",                  layout: "columns" },
  { id: "glass-dock",   name: "Verre & dock",  tagline: "Panneaux de verre et dock de catégories en bas",                      layout: "dock" },
  { id: "mission",      name: "Mission",       tagline: "Onglets d'outils ouverts et barre de sections à menus",               layout: "tabs" },
];

export const DEFAULT_UI_MODEL: UiModelId = "forge";
const STORAGE_KEY = "nitrite-ui-model";

export function isUiModelId(value: unknown): value is UiModelId {
  return typeof value === "string" && UI_MODELS.some(m => m.id === value);
}

export const useUiModelStore = defineStore("uiModel", () => {
  const model = ref<UiModelId>(DEFAULT_UI_MODEL);
  const info = computed(() => UI_MODELS.find(m => m.id === model.value) ?? UI_MODELS[0]);

  function apply() {
    document.documentElement.setAttribute("data-ui-model", model.value);
  }

  function setModel(id: UiModelId) {
    model.value = isUiModelId(id) ? id : DEFAULT_UI_MODEL;
    try { localStorage.setItem(STORAGE_KEY, model.value); } catch { /* stockage indisponible */ }
    apply();
  }

  function load() {
    let saved: string | null = null;
    try { saved = localStorage.getItem(STORAGE_KEY); } catch { /* stockage indisponible */ }
    model.value = isUiModelId(saved) ? saved : DEFAULT_UI_MODEL;
    apply();
  }

  return { model, info, setModel, load };
});
