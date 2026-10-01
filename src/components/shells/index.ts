import type { Component } from "vue";
import type { UiModelId } from "@/stores/uiModel";
import ClassicShell from "./ClassicShell.vue";

/** Coque de chaque modèle d'interface. Un modèle inconnu retombe sur Classique. */
const shells: Partial<Record<UiModelId, Component>> = {
  classic: ClassicShell,
};

export function shellFor(id: UiModelId): Component {
  return shells[id] ?? ClassicShell;
}
