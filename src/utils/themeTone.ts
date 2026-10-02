/** Tonalité claire / sombre d'un thème (module sans dépendance, évite les imports circulaires). */

/** Vrai si la couleur (#rrggbb) est claire (luminance relative > 0,5). */
export function isLightColor(hex: string): boolean {
  const m = hex.trim().replace("#", "").match(/^([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  if (!m) return false;
  const [r, g, b] = [m[1], m[2], m[3]].map((c) => {
    const v = parseInt(c, 16) / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.5;
}

/**
 * Pose `data-theme-tone` sur <html> : les modèles d'interface et les styles
 * globaux adaptent ombres, verre et halos aux thèmes clairs.
 */
export function applyThemeTone(tone: "light" | "dark"): void {
  document.documentElement.setAttribute("data-theme-tone", tone);
}
