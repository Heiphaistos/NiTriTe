import { onUnmounted, watch, type Ref } from "vue";

/**
 * Navigation clavier dans une barre ou un menu de coque :
 * flèches (selon l'orientation), Début / Fin pour aller d'un élément focusable
 * à l'autre parmi ceux qui correspondent à `selector`.
 */
export function useMenuKeys(
  container: Ref<HTMLElement | null>,
  selector: string,
  orientation: "horizontal" | "vertical" | "both" = "both",
) {
  function onKey(e: KeyboardEvent) {
    const root = container.value;
    if (!root) return;
    const items = Array.from(root.querySelectorAll<HTMLElement>(selector)).filter(el => el.offsetParent !== null || el === document.activeElement);
    const idx = items.indexOf(document.activeElement as HTMLElement);
    if (idx < 0) return;
    const next = orientation !== "vertical" ? ["ArrowRight"] : [];
    const prev = orientation !== "vertical" ? ["ArrowLeft"] : [];
    if (orientation !== "horizontal") { next.push("ArrowDown"); prev.push("ArrowUp"); }
    let target = -1;
    if (next.includes(e.key)) target = (idx + 1) % items.length;
    else if (prev.includes(e.key)) target = (idx - 1 + items.length) % items.length;
    else if (e.key === "Home") target = 0;
    else if (e.key === "End") target = items.length - 1;
    if (target < 0) return;
    e.preventDefault();
    items[target].focus();
  }
  // Le conteneur peut apparaître plus tard (menu en v-if) : on suit la référence.
  watch(container, (el, old) => {
    old?.removeEventListener("keydown", onKey);
    el?.addEventListener("keydown", onKey);
  }, { immediate: true, flush: "post" });
  onUnmounted(() => container.value?.removeEventListener("keydown", onKey));
}

/** Donne le focus au premier élément d'un menu qui vient de s'ouvrir. */
export function focusFirst(root: () => HTMLElement | null | undefined, selector: string) {
  // Après le rendu : le menu vient souvent d'apparaître (v-if).
  requestAnimationFrame(() => root()?.querySelector<HTMLElement>(selector)?.focus());
}
