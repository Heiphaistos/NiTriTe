import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { readHardwareHints, resolveTier, type PerfMode, type PerfTier } from "@/utils/perfProfile";

export type ThemeName =
  | "nitrite-dark" | "cyber-blue" | "matrix-green" | "purple-haze" | "red-alert"
  | "arctic-light" | "midnight-gold" | "neon-synthwave" | "ocean-deep" | "rose-quartz"
  | "void-dark" | "forest-green" | "copper-rust" | "slate-steel"
  | "inferno" | "aurora" | "moonlight" | "ember-glow" | "cobalt-night"
  | "volcanic" | "sakura" | "jade-temple" | "hacker" | "ice-storm"
  | "custom";

export const useAppStore = defineStore("app", () => {
  const theme = ref<ThemeName>("nitrite-dark");
  const sidebarCollapsed = ref(false);
  const language = ref<"fr" | "en">("fr");
  const fontSize = ref<"small" | "normal" | "large">("normal");
  const showAnimations = ref(true);
  const perfMode = ref<PerfMode>("auto");
  const hardware = readHardwareHints();
  const perfTier = computed<PerfTier>(() => resolveTier(perfMode.value, hardware));

  /** Pose `data-perf` sur <html> : performance.css en derive les coupes d'effets. */
  function applyPerf() {
    document.documentElement.setAttribute("data-perf", perfTier.value);
  }

  function setPerfMode(mode: PerfMode) {
    perfMode.value = mode;
    try { localStorage.setItem("nitrite-perf-mode", mode); } catch { /* stockage indisponible */ }
    applyPerf();
  }

  function loadPerfMode() {
    let saved: string | null = null;
    try { saved = localStorage.getItem("nitrite-perf-mode"); } catch { /* stockage indisponible */ }
    if (saved === "auto" || saved === "full" || saved === "balanced" || saved === "light") perfMode.value = saved;
    applyPerf();
  }

  function setTheme(name: ThemeName) {
    // Fondu de 300 ms sur CHAQUE element : trop lourd pour le profil leger.
    const animate = perfTier.value !== "light";
    if (animate) document.documentElement.classList.add("theme-transitioning");
    document.documentElement.setAttribute("data-theme", name);
    theme.value = name;
    localStorage.setItem("nitrite-theme", name);
    setTimeout(() => {
      document.documentElement.classList.remove("theme-transitioning");
    }, 350);
  }

  function loadSavedTheme() {
    const saved = localStorage.getItem("nitrite-theme") as ThemeName | null;
    if (saved) setTheme(saved);
  }

  function toggleSidebar() {
    sidebarCollapsed.value = !sidebarCollapsed.value;
    localStorage.setItem("nitrite-sidebar", String(sidebarCollapsed.value));
  }

  function loadSidebarState() {
    const saved = localStorage.getItem("nitrite-sidebar");
    if (saved === "true") sidebarCollapsed.value = true;
  }

  function setFontSize(size: "small" | "normal" | "large") {
    fontSize.value = size;
    document.documentElement.setAttribute("data-font-size", size);
    localStorage.setItem("nitrite-font-size", size);
  }

  function loadFontSize() {
    const saved = localStorage.getItem("nitrite-font-size") as "small" | "normal" | "large" | null;
    if (saved) setFontSize(saved);
  }

  function toggleAnimations() {
    showAnimations.value = !showAnimations.value;
    if (showAnimations.value) {
      document.documentElement.classList.remove("no-animations");
    } else {
      document.documentElement.classList.add("no-animations");
    }
    localStorage.setItem("nitrite-animations", String(showAnimations.value));
  }

  function loadAnimations() {
    const saved = localStorage.getItem("nitrite-animations");
    if (saved === "false") {
      showAnimations.value = false;
      document.documentElement.classList.add("no-animations");
    }
  }

  return {
    theme,
    sidebarCollapsed,
    language,
    fontSize,
    showAnimations,
    perfMode,
    perfTier,
    hardware,
    setPerfMode,
    loadPerfMode,
    setTheme,
    loadSavedTheme,
    toggleSidebar,
    loadSidebarState,
    setFontSize,
    loadFontSize,
    toggleAnimations,
    loadAnimations,
  };
});
