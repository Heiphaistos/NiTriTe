/**
 * Catalogue unique des themes : les 24 themes CSS (`themes.css`) et les
 * presets a variables de l'editeur (Windows 11, Dracula, Nord, Nouveautes…).
 *
 * Avant, Parametres ne proposait que les 24 themes CSS ; les autres n'etaient
 * accessibles que par l'apercu de l'editeur, et disparaissaient au redemarrage.
 * Un preset s'applique desormais comme un theme normal, et persiste.
 */
import { PRESET_THEMES, PRESET_THEME_GROUPS } from "@/stores/themeEditor";

export interface ThemeEntry {
  id: string;
  label: string;
  accent: string;
  background: string;
  /** Presets a variables : injectees sur <html>, data-theme="custom". */
  vars?: Record<string, string>;
}

export interface ThemeGroup {
  label: string;
  themes: ThemeEntry[];
}

export const CSS_THEMES: ThemeEntry[] = [
  { id: "nitrite-dark",   label: "Nitrite Dark",       accent: "#f97316", background: "#09090b" },
  { id: "cyber-blue",     label: "Cyber Blue",         accent: "#3b82f6", background: "#0a0a1a" },
  { id: "matrix-green",   label: "Matrix Green",       accent: "#22c55e", background: "#030a03" },
  { id: "purple-haze",    label: "Purple Haze",        accent: "#a855f7", background: "#0a0510" },
  { id: "red-alert",      label: "Red Alert",          accent: "#ef4444", background: "#0a0505" },
  { id: "arctic-light",   label: "Arctic Light",       accent: "#0ea5e9", background: "#f8fafc" },
  { id: "midnight-gold",  label: "Midnight Gold",      accent: "#eab308", background: "#0a0a05" },
  { id: "neon-synthwave", label: "Neon Synthwave",     accent: "#f0abfc", background: "#0d0015" },
  { id: "ocean-deep",     label: "Ocean Deep",         accent: "#06b6d4", background: "#020f1a" },
  { id: "rose-quartz",    label: "Rose Quartz",        accent: "#f43f5e", background: "#1a0a10" },
  { id: "void-dark",      label: "Void Dark (AMOLED)", accent: "#6366f1", background: "#000000" },
  { id: "forest-green",   label: "Forest Green",       accent: "#16a34a", background: "#040f06" },
  { id: "copper-rust",    label: "Copper Rust",        accent: "#d97706", background: "#120a05" },
  { id: "slate-steel",    label: "Slate Steel",        accent: "#64748b", background: "#0a0e18" },
  { id: "inferno",        label: "Inferno",            accent: "#ff4500", background: "#0c0200" },
  { id: "aurora",         label: "Aurora Borealis",    accent: "#00d4aa", background: "#020d18" },
  { id: "moonlight",      label: "Moonlight",          accent: "#7dd3fc", background: "#0c1220" },
  { id: "ember-glow",     label: "Ember Glow",         accent: "#fb923c", background: "#100800" },
  { id: "cobalt-night",   label: "Cobalt Night",       accent: "#2563eb", background: "#05091a" },
  { id: "volcanic",       label: "Volcanic",           accent: "#f97316", background: "#0a0806" },
  { id: "sakura",         label: "Sakura (Clair)",     accent: "#ec4899", background: "#fff8fb" },
  { id: "jade-temple",    label: "Jade Temple",        accent: "#10b981", background: "#03100a" },
  { id: "hacker",         label: "Hacker Terminal",    accent: "#00ff41", background: "#000000" },
  { id: "ice-storm",      label: "Ice Storm (Clair)",  accent: "#0284c7", background: "#f0f9ff" },
];

const CSS_IDS = new Set(CSS_THEMES.map((t) => t.id));

export function isCssTheme(id: string): boolean {
  return CSS_IDS.has(id) || id === "custom";
}

/** Presets a variables qui ne sont pas deja des themes CSS. */
export const VAR_THEMES: ThemeEntry[] = PRESET_THEMES
  .filter((p) => !CSS_IDS.has(p.id))
  .map((p) => ({ id: p.id, label: p.label, accent: p.accent, background: p.vars["--bg-primary"] ?? "#000", vars: p.vars }));

export function findVarTheme(id: string): ThemeEntry | undefined {
  return VAR_THEMES.find((t) => t.id === id);
}

/** Galerie groupee pour Parametres : Nitrite (CSS) puis groupes de l'editeur. */
export function themeGallery(): ThemeGroup[] {
  const byId = new Map(VAR_THEMES.map((t) => [t.id, t]));
  const groups: ThemeGroup[] = [{ label: "🎨 Nitrite", themes: CSS_THEMES }];
  for (const g of PRESET_THEME_GROUPS) {
    const themes = g.ids.map((id) => byId.get(id)).filter((t): t is ThemeEntry => !!t);
    if (themes.length) groups.push({ label: g.label, themes });
  }
  return groups;
}

export function totalThemeCount(): number {
  return CSS_THEMES.length + VAR_THEMES.length;
}

// ── Application des variables ────────────────────────────────────────────────

function hexToRgb(hex: string): string | null {
  const m = hex.trim().replace("#", "").match(/^([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  if (!m) return null;
  return `${parseInt(m[1], 16)}, ${parseInt(m[2], 16)}, ${parseInt(m[3], 16)}`;
}

/** Variables derivees calculees a partir des couleurs de base. */
export function deriveThemeVars(vars: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = { ...vars };
  const accent = hexToRgb(vars["--accent-primary"] ?? "");
  if (accent) {
    out["--accent-muted"] = `rgba(${accent}, 0.12)`;
    out["--accent-subtle"] = `rgba(${accent}, 0.06)`;
    out["--accent-glow"] = `0 0 24px rgba(${accent}, 0.4)`;
    out["--accent-glow-sm"] = `0 0 10px rgba(${accent}, 0.3)`;
  }
  for (const k of ["success", "warning", "danger", "info"]) {
    const rgb = hexToRgb(vars[`--${k}`] ?? "");
    if (rgb) out[`--${k}-muted`] = `rgba(${rgb}, 0.12)`;
  }
  if (vars["--border-hover"] && !vars["--border-strong"]) out["--border-strong"] = vars["--border-hover"];
  return out;
}

const APPLIED_KEY = "__nitriteThemeVars";

/** Pose les variables sur <html> (et retire celles du theme precedent). */
export function applyThemeVars(vars: Record<string, string>): void {
  clearThemeVars();
  const all = deriveThemeVars(vars);
  const root = document.documentElement;
  for (const [k, v] of Object.entries(all)) {
    if (k.startsWith("--")) root.style.setProperty(k, v);
  }
  (root as unknown as Record<string, unknown>)[APPLIED_KEY] = Object.keys(all);
}

export function clearThemeVars(): void {
  const root = document.documentElement;
  const keys = (root as unknown as Record<string, unknown>)[APPLIED_KEY] as string[] | undefined;
  for (const k of keys ?? []) root.style.removeProperty(k);
  (root as unknown as Record<string, unknown>)[APPLIED_KEY] = [];
}
