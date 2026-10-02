<script setup lang="ts">
/**
 * Barre de mission : visible au-dessus de chaque page tant qu'une mission est
 * en cours, dans tous les modèles d'interface. Elle indique l'étape à faire et
 * enchaîne automatiquement vers l'outil suivant quand l'étape est terminée.
 */
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { Flag, Check, SkipForward, ArrowRight, ListChecks } from "lucide-vue-next";
import { useMissionStore, toolLabel } from "@/stores/mission";

const mission = useMissionStore();
const route = useRoute();
const router = useRouter();

const visible = computed(() => !!mission.active && route.path !== "/missions");
const onStep = computed(() => mission.currentStep?.route === route.path);

function finishStep(status: "done" | "skipped") {
  const next = mission.complete(status);
  if (next) router.push(next.route);
}
</script>

<template>
  <div v-if="visible && mission.active" class="mbar" role="region" aria-label="Mission en cours">
    <span class="mbar__kicker"><Flag :size="12" /> Mission</span>
    <span class="mbar__name">{{ mission.active.name }}<template v-if="mission.active.client"> · {{ mission.active.client }}</template></span>

    <template v-if="!mission.isFinished && mission.currentStep">
      <span class="mbar__step">
        Étape {{ mission.active.current + 1 }}/{{ mission.active.steps.length }} :
        <strong>{{ toolLabel(mission.currentStep.route) }}</strong>
        <span v-if="mission.currentStep.hint && onStep" class="mbar__hint">&nbsp;— {{ mission.currentStep.hint }}</span>
      </span>
      <span class="mbar__progress" :aria-label="`Progression ${mission.progress} %`"><span :style="{ width: `${mission.progress}%` }" /></span>
      <button v-if="!onStep" class="mbar__btn mbar__btn--primary" @click="router.push(mission.currentStep.route)">
        Aller à l'étape <ArrowRight :size="13" />
      </button>
      <template v-else>
        <button class="mbar__btn mbar__btn--primary" title="Marquer l'étape comme faite et passer à la suivante" @click="finishStep('done')">
          <Check :size="13" /> Étape terminée
        </button>
        <button class="mbar__btn" title="Passer cette étape" @click="finishStep('skipped')">
          <SkipForward :size="13" /> Passer
        </button>
      </template>
    </template>
    <span v-else class="mbar__step"><strong>Mission terminée</strong> — rapport prêt</span>

    <button class="mbar__btn" title="Détail de la mission et rapport" @click="router.push('/missions')">
      <ListChecks :size="13" /> Détails
    </button>
  </div>
</template>

<style scoped>
.mbar {
  position: sticky; top: 0; z-index: 40;
  display: flex; align-items: center; gap: 12px; flex-wrap: wrap;
  margin-bottom: 14px; padding: 8px 12px;
  border-radius: var(--radius-md); border: 1px solid color-mix(in srgb, var(--accent-primary) 40%, var(--border));
  background: color-mix(in srgb, var(--accent-primary) 8%, var(--bg-secondary));
  box-shadow: var(--shadow-md); font-size: 12.5px;
}
.mbar__kicker {
  display: inline-flex; align-items: center; gap: 5px; padding: 2px 8px; border-radius: 999px;
  background: var(--accent-primary); color: var(--bg-primary);
  font-size: 10.5px; font-weight: 800; letter-spacing: 0.08em; text-transform: uppercase;
}
.mbar__name { font-weight: 700; white-space: nowrap; }
.mbar__step { color: var(--text-secondary); min-width: 0; flex: 1; }
.mbar__step strong { color: var(--text-primary); }
.mbar__hint { color: var(--text-muted); }
.mbar__progress { width: 120px; height: 5px; border-radius: 99px; background: var(--bg-tertiary); overflow: hidden; flex-shrink: 0; }
.mbar__progress span { display: block; height: 100%; background: var(--accent-primary); transition: width var(--transition-normal); }
.mbar__btn {
  display: inline-flex; align-items: center; gap: 6px; height: 28px; padding: 0 11px; flex-shrink: 0;
  border-radius: var(--radius-sm); border: 1px solid var(--border); background: var(--bg-primary);
  color: var(--text-primary); font-family: inherit; font-size: 12px; font-weight: 600; cursor: pointer;
}
.mbar__btn:hover { border-color: var(--accent-primary); }
.mbar__btn--primary { background: var(--accent-primary); border-color: var(--accent-primary); color: var(--bg-primary); }
.mbar__btn--primary:hover { background: var(--accent-hover); }
</style>
