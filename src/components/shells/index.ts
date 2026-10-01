import type { Component } from "vue";
import type { UiModelId } from "@/stores/uiModel";
import ClassicShell from "./ClassicShell.vue";
import ForgeShell from "./ForgeShell.vue";
import CommandDeckShell from "./CommandDeckShell.vue";
import BentoShell from "./BentoShell.vue";
import ConsoleShell from "./ConsoleShell.vue";
import OrbitalShell from "./OrbitalShell.vue";
import ColumnsShell from "./ColumnsShell.vue";
import GlassDockShell from "./GlassDockShell.vue";
import MissionShell from "./MissionShell.vue";

/** Coque de chaque modèle d'interface. Un modèle inconnu retombe sur Forge. */
export const shells: Record<UiModelId, Component> = {
  "forge": ForgeShell,
  "classic": ClassicShell,
  "command-deck": CommandDeckShell,
  "bento": BentoShell,
  "console": ConsoleShell,
  "orbital": OrbitalShell,
  "columns": ColumnsShell,
  "glass-dock": GlassDockShell,
  "mission": MissionShell,
};

export function shellFor(id: UiModelId): Component {
  return shells[id] ?? ForgeShell;
}
