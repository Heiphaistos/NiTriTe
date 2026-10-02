<script setup lang="ts">
/**
 * Mode Mission : enchaîner plusieurs outils pour une intervention (remise en
 * état, PC lent, PC neuf…), suivre chaque étape et produire le rapport.
 * Chaque étape ouvre la page existante de l'outil, avec ses propres boutons.
 */
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import {
  Flag, Play, Check, SkipForward, RotateCcw, ArrowRight, Copy, Download,
  Plus, Trash2, Pencil, ChevronUp, ChevronDown, X, History, Square,
} from "lucide-vue-next";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import { useMissionStore, toolLabel, missionReport, type ActiveMission } from "@/stores/mission";
import type { MissionTemplate } from "@/data/missions";
import { navigationSections } from "@/data/navigation";
import { getNavIcon } from "@/data/navIcons";
import { useClipboard } from "@/composables/useClipboard";
import { useExportData } from "@/composables/useExportData";
import { useNotificationStore } from "@/stores/notifications";

const mission = useMissionStore();
const router = useRouter();
const { copy } = useClipboard();
const { exportTXT } = useExportData();
const notify = useNotificationStore();

const client = ref("");
const allTools = navigationSections.flatMap(s => s.items).filter(i => i.route !== "/missions");
function iconOf(route: string) { return getNavIcon(allTools.find(i => i.route === route)?.icon ?? ""); }

function startMission(t: MissionTemplate) {
  const first = mission.start(t.id, client.value);
  client.value = "";
  if (first) router.push(first.route);
}

function finishStep(status: "done" | "skipped") {
  const next = mission.complete(status);
  if (next) router.push(next.route);
}

async function copyReport(m: ActiveMission) {
  const ok = await copy(missionReport(m).join("\n"));
  if (ok) notify.success("Mission", "Rapport copié dans le presse-papiers");
  else notify.error("Mission", "Copie impossible");
}
function exportReport(m: ActiveMission) {
  const date = new Date(m.startedAt).toISOString().slice(0, 10);
  exportTXT(missionReport(m), `mission-${m.templateId}-${date}`);
}

// ── Éditeur de modèles personnalisés ──
const editing = ref<{ id?: string; name: string; description: string; routes: string[] } | null>(null);
function newTemplate() { editing.value = { name: "", description: "", routes: [] }; }
function editTemplate(t: MissionTemplate) {
  editing.value = { id: t.id, name: t.name, description: t.description, routes: t.steps.map(s => s.route) };
}
function toggleRoute(route: string) {
  if (!editing.value) return;
  const r = editing.value.routes;
  editing.value.routes = r.includes(route) ? r.filter(x => x !== route) : [...r, route];
}
function move(i: number, d: -1 | 1) {
  const r = editing.value?.routes;
  if (!r || i + d < 0 || i + d >= r.length) return;
  [r[i], r[i + d]] = [r[i + d], r[i]];
}
const canSave = computed(() => !!editing.value && editing.value.name.trim().length > 0 && editing.value.routes.length > 0);
function saveTemplate() {
  const e = editing.value;
  if (!e || !mission.saveTemplate(e.name, e.description, e.routes, e.id)) return;
  notify.success("Mission", `Modèle « ${e.name.trim()} » enregistré`);
  editing.value = null;
}
</script>

<template>
  <div class="missions">
    <div class="page-header">
      <div>
        <h1>Missions</h1>
        <p class="page-subtitle">Enchaînez les outils d'une intervention, étape par étape, et obtenez le rapport à la fin.</p>
      </div>
    </div>

    <!-- ── Mission en cours ── -->
    <NCard v-if="mission.active" class="ms-active">
      <template #header>
        <div class="ms-head">
          <Flag :size="16" />
          <span>{{ mission.active.name }}</span>
          <NBadge :variant="mission.isFinished ? 'success' : 'accent'">{{ mission.isFinished ? "Terminée" : "En cours" }}</NBadge>
          <span v-if="mission.active.client" class="ms-client">{{ mission.active.client }}</span>
          <span class="ms-spacer" />
          <span class="ms-pct">{{ mission.progress }} %</span>
        </div>
      </template>
      <div class="ms-progress"><span :style="{ width: `${mission.progress}%` }" /></div>

      <ol class="ms-steps">
        <li
          v-for="(s, i) in mission.active.steps"
          :key="i"
          class="ms-step"
          :class="[`is-${s.status}`, { current: i === mission.active.current && !mission.isFinished }]"
        >
          <span class="ms-step__n">
            <Check v-if="s.status === 'done'" :size="13" />
            <SkipForward v-else-if="s.status === 'skipped'" :size="12" />
            <template v-else>{{ i + 1 }}</template>
          </span>
          <component :is="iconOf(s.route)" :size="16" class="ms-step__icon" />
          <div class="ms-step__body">
            <span class="ms-step__label">{{ toolLabel(s.route) }}</span>
            <span v-if="s.hint" class="ms-step__hint">{{ s.hint }}</span>
            <input
              class="ms-step__note"
              :value="s.note ?? ''"
              placeholder="Note pour le rapport (facultatif)"
              @input="mission.setNote(i, ($event.target as HTMLInputElement).value)"
            />
          </div>
          <div class="ms-step__actions">
            <NButton size="sm" variant="secondary" @click="mission.goTo(i); router.push(s.route)">
              Ouvrir <ArrowRight :size="13" />
            </NButton>
            <template v-if="i === mission.active.current && s.status === 'pending'">
              <NButton size="sm" variant="primary" @click="finishStep('done')"><Check :size="13" /> Fait</NButton>
              <NButton size="sm" variant="ghost" @click="finishStep('skipped')"><SkipForward :size="13" /> Passer</NButton>
            </template>
            <NButton v-else-if="s.status !== 'pending'" size="sm" variant="ghost" title="Refaire cette étape" @click="mission.reopen(i)">
              <RotateCcw :size="13" />
            </NButton>
          </div>
        </li>
      </ol>

      <div class="ms-foot">
        <NButton variant="secondary" size="sm" @click="copyReport(mission.active)"><Copy :size="13" /> Copier le rapport</NButton>
        <NButton variant="secondary" size="sm" @click="exportReport(mission.active)"><Download :size="13" /> Exporter (.txt)</NButton>
        <span class="ms-spacer" />
        <NButton :variant="mission.isFinished ? 'primary' : 'danger'" size="sm" @click="mission.stop()">
          <Square :size="13" /> {{ mission.isFinished ? "Clore la mission" : "Abandonner" }}
        </NButton>
      </div>
    </NCard>

    <!-- ── Modèles ── -->
    <div class="ms-section">
      <h2 class="section-header">Démarrer une mission</h2>
      <label class="ms-client-field">
        <span>Client / poste (facultatif)</span>
        <input v-model="client" placeholder="ex. PC de Julie — Dell Inspiron" :disabled="!!mission.active" />
      </label>
      <p v-if="mission.active" class="ms-muted">Terminez ou abandonnez la mission en cours pour en démarrer une autre.</p>
      <div class="ms-grid">
        <NCard v-for="t in mission.templates" :key="t.id" class="ms-tpl">
          <div class="ms-tpl__head">
            <span class="ms-tpl__name">{{ t.name }}</span>
            <NBadge v-if="t.custom" variant="info">Perso</NBadge>
          </div>
          <p class="ms-tpl__desc">{{ t.description || `${t.steps.length} étapes` }}</p>
          <div class="ms-tpl__chain">
            <span v-for="(s, i) in t.steps" :key="i" class="ms-chip">
              <component :is="iconOf(s.route)" :size="12" /> {{ toolLabel(s.route) }}
            </span>
          </div>
          <div class="ms-tpl__actions">
            <NButton size="sm" :disabled="!!mission.active" @click="startMission(t)"><Play :size="13" /> Démarrer</NButton>
            <template v-if="t.custom">
              <NButton size="sm" variant="ghost" title="Modifier" @click="editTemplate(t)"><Pencil :size="13" /></NButton>
              <NButton size="sm" variant="ghost" title="Supprimer" @click="mission.deleteTemplate(t.id)"><Trash2 :size="13" /></NButton>
            </template>
          </div>
        </NCard>
        <button class="ms-new" @click="newTemplate"><Plus :size="18" /> Créer un modèle de mission</button>
      </div>
    </div>

    <!-- ── Éditeur ── -->
    <NCard v-if="editing" class="ms-editor">
      <template #header>
        <div class="ms-head">
          <Pencil :size="15" /><span>{{ editing.id ? "Modifier le modèle" : "Nouveau modèle de mission" }}</span>
          <span class="ms-spacer" />
          <button class="ms-x" aria-label="Fermer" @click="editing = null"><X :size="15" /></button>
        </div>
      </template>
      <div class="ms-editor__fields">
        <label><span>Nom</span><input v-model="editing.name" placeholder="ex. Révision annuelle" /></label>
        <label><span>Description</span><input v-model="editing.description" placeholder="À quoi sert cette mission ?" /></label>
      </div>
      <div class="ms-editor__cols">
        <div class="ms-editor__pick">
          <div v-for="sec in navigationSections" :key="sec.title" class="ms-pick-sec">
            <span class="ms-pick-sec__title">{{ sec.title }}</span>
            <label v-for="it in sec.items.filter(x => x.route !== '/missions')" :key="it.id" class="ms-pick">
              <input type="checkbox" :checked="editing.routes.includes(it.route)" @change="toggleRoute(it.route)" />
              {{ it.label }}
            </label>
          </div>
        </div>
        <div class="ms-editor__order">
          <span class="ms-pick-sec__title">Ordre des étapes ({{ editing.routes.length }})</span>
          <p v-if="!editing.routes.length" class="ms-muted">Cochez les outils à enchaîner.</p>
          <div v-for="(r, i) in editing.routes" :key="r" class="ms-order">
            <span class="ms-step__n">{{ i + 1 }}</span>
            <span class="ms-order__label">{{ toolLabel(r) }}</span>
            <button :disabled="i === 0" aria-label="Monter" @click="move(i, -1)"><ChevronUp :size="14" /></button>
            <button :disabled="i === editing.routes.length - 1" aria-label="Descendre" @click="move(i, 1)"><ChevronDown :size="14" /></button>
            <button aria-label="Retirer" @click="toggleRoute(r)"><X :size="14" /></button>
          </div>
        </div>
      </div>
      <div class="ms-foot">
        <span class="ms-spacer" />
        <NButton variant="ghost" size="sm" @click="editing = null">Annuler</NButton>
        <NButton size="sm" :disabled="!canSave" @click="saveTemplate">Enregistrer</NButton>
      </div>
    </NCard>

    <!-- ── Historique ── -->
    <div v-if="mission.history.length" class="ms-section">
      <h2 class="section-header"><History :size="15" /> Missions précédentes</h2>
      <div class="ms-history">
        <div v-for="(h, i) in mission.history" :key="i" class="ms-hist">
          <span class="ms-hist__name">{{ h.name }}<template v-if="h.client"> · {{ h.client }}</template></span>
          <span class="ms-muted">{{ new Date(h.startedAt).toLocaleString("fr-FR", { dateStyle: "short", timeStyle: "short" }) }}</span>
          <span class="ms-muted">{{ h.steps.filter(s => s.status === "done").length }}/{{ h.steps.length }} faites</span>
          <span class="ms-spacer" />
          <NButton size="sm" variant="ghost" @click="copyReport(h)"><Copy :size="13" /> Rapport</NButton>
          <NButton size="sm" variant="ghost" @click="exportReport(h)"><Download :size="13" /></NButton>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.missions { display: flex; flex-direction: column; gap: 20px; }
.page-header h1 { font-size: 24px; font-weight: 700; }
.page-subtitle { color: var(--text-secondary); font-size: 13px; margin-top: 2px; }
.section-header { display: flex; align-items: center; gap: 8px; font-size: 15px; font-weight: 700; margin-bottom: 10px; }
.ms-head { display: flex; align-items: center; gap: 10px; width: 100%; }
.ms-client { color: var(--text-secondary); font-weight: 500; font-size: 13px; }
.ms-spacer { flex: 1; }
.ms-pct { font-family: "JetBrains Mono", monospace; color: var(--accent-primary); }
.ms-muted { color: var(--text-muted); font-size: 12px; }
.ms-progress { height: 6px; border-radius: 99px; background: var(--bg-tertiary); overflow: hidden; margin-bottom: 14px; }
.ms-progress span { display: block; height: 100%; background: var(--accent-primary); transition: width var(--transition-normal); }

.ms-steps { list-style: none; display: flex; flex-direction: column; gap: 6px; padding: 0; margin: 0; }
.ms-step {
  display: flex; align-items: flex-start; gap: 12px; padding: 10px 12px;
  border: 1px solid var(--border); border-radius: var(--radius-md); background: var(--bg-primary);
}
.ms-step.current { border-color: var(--accent-primary); box-shadow: 0 0 0 1px var(--accent-primary); }
.ms-step.is-done, .ms-step.is-skipped { opacity: 0.7; }
.ms-step__n {
  width: 22px; height: 22px; flex-shrink: 0; border-radius: 50%; display: grid; place-items: center;
  font-size: 11px; font-weight: 700; background: var(--bg-tertiary); color: var(--text-secondary);
}
.ms-step.current .ms-step__n { background: var(--accent-primary); color: var(--bg-primary); }
.ms-step.is-done .ms-step__n { background: var(--success); color: var(--bg-primary); }
.ms-step__icon { color: var(--accent-primary); margin-top: 3px; flex-shrink: 0; }
.ms-step__body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
.ms-step__label { font-weight: 700; }
.ms-step__hint { font-size: 12px; color: var(--text-muted); }
.ms-step__note, .ms-client-field input, .ms-editor__fields input {
  height: 30px; padding: 0 10px; border-radius: var(--radius-sm); border: 1px solid var(--border);
  background: var(--bg-secondary); color: var(--text-primary); font-family: inherit; font-size: 12.5px;
}
.ms-step__note { margin-top: 4px; max-width: 520px; }
.ms-step__actions { display: flex; gap: 6px; flex-shrink: 0; }
.ms-foot { display: flex; align-items: center; gap: 8px; margin-top: 14px; }

.ms-client-field { display: flex; flex-direction: column; gap: 4px; max-width: 420px; margin-bottom: 12px; font-size: 12px; color: var(--text-secondary); }
.ms-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 12px; }
.ms-tpl__head { display: flex; align-items: center; gap: 8px; }
.ms-tpl__name { font-weight: 800; font-size: 14.5px; }
.ms-tpl__desc { font-size: 12.5px; color: var(--text-secondary); margin: 6px 0 10px; }
.ms-tpl__chain { display: flex; flex-wrap: wrap; gap: 5px; margin-bottom: 12px; }
.ms-chip {
  display: inline-flex; align-items: center; gap: 5px; padding: 3px 8px; border-radius: 999px;
  background: var(--bg-tertiary); color: var(--text-secondary); font-size: 11px;
}
.ms-chip :deep(svg) { color: var(--accent-primary); }
.ms-tpl__actions { display: flex; gap: 6px; }
.ms-new {
  display: flex; align-items: center; justify-content: center; gap: 8px; min-height: 150px;
  border: 1px dashed var(--border-hover); border-radius: var(--radius-xl); background: transparent;
  color: var(--text-secondary); font-family: inherit; font-size: 13px; font-weight: 600; cursor: pointer;
}
.ms-new:hover { color: var(--accent-primary); border-color: var(--accent-primary); }

.ms-x { border: none; background: transparent; color: var(--text-muted); cursor: pointer; }
.ms-editor__fields { display: grid; grid-template-columns: 1fr 2fr; gap: 10px; margin-bottom: 14px; }
.ms-editor__fields label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--text-secondary); }
.ms-editor__cols { display: grid; grid-template-columns: 2fr 1fr; gap: 16px; }
.ms-editor__pick { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 12px; max-height: 360px; overflow-y: auto; }
.ms-pick-sec { display: flex; flex-direction: column; gap: 3px; }
.ms-pick-sec__title { font-size: 10.5px; letter-spacing: 0.1em; text-transform: uppercase; color: var(--text-muted); margin-bottom: 3px; }
.ms-pick { display: flex; align-items: center; gap: 7px; font-size: 12.5px; cursor: pointer; }
.ms-editor__order { display: flex; flex-direction: column; gap: 5px; }
.ms-order { display: flex; align-items: center; gap: 8px; padding: 5px 8px; border-radius: var(--radius-sm); background: var(--bg-tertiary); font-size: 12.5px; }
.ms-order__label { flex: 1; }
.ms-order button { border: none; background: transparent; color: var(--text-secondary); cursor: pointer; padding: 2px; }
.ms-order button:disabled { opacity: 0.3; cursor: default; }

.ms-history { display: flex; flex-direction: column; gap: 4px; }
.ms-hist { display: flex; align-items: center; gap: 12px; padding: 8px 12px; border-radius: var(--radius-md); background: var(--bg-secondary); border: 1px solid var(--border); }
.ms-hist__name { font-weight: 600; }
@media (max-width: 1100px) { .ms-editor__cols { grid-template-columns: 1fr; } }
</style>
