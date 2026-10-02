import { defineStore } from "pinia";
import { computed, ref, watch } from "vue";
import { BUILTIN_MISSIONS, type MissionTemplate, type MissionStepTemplate } from "@/data/missions";
import { navigationSections } from "@/data/navigation";

export type StepStatus = "pending" | "done" | "skipped";

export interface MissionStep extends MissionStepTemplate {
  status: StepStatus;
  finishedAt?: string;
  note?: string;
}

export interface ActiveMission {
  templateId: string;
  name: string;
  client: string;
  startedAt: string;
  finishedAt?: string;
  current: number;
  steps: MissionStep[];
}

const ACTIVE_KEY = "nitrite-mission-active";
const CUSTOM_KEY = "nitrite-mission-templates";
const HISTORY_KEY = "nitrite-mission-history";
const HISTORY_MAX = 20;

const knownRoutes = new Set(navigationSections.flatMap(s => s.items.map(i => i.route)));

export function toolLabel(route: string): string {
  for (const s of navigationSections) for (const i of s.items) if (i.route === route) return i.label;
  return route;
}

function read<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch { return fallback; }
}
function write(key: string, value: unknown) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, JSON.stringify(value));
  } catch { /* stockage indisponible */ }
}

/** Mode Mission : enchaînement guidé d'outils, suivi des étapes et rapport. */
export const useMissionStore = defineStore("mission", () => {
  const active = ref<ActiveMission | null>(read<ActiveMission | null>(ACTIVE_KEY, null));
  const customTemplates = ref<MissionTemplate[]>(read<MissionTemplate[]>(CUSTOM_KEY, []));
  const history = ref<ActiveMission[]>(read<ActiveMission[]>(HISTORY_KEY, []));

  // Données corrompues ou routes disparues : on les écarte.
  if (active.value && !Array.isArray(active.value.steps)) active.value = null;
  customTemplates.value = customTemplates.value.filter(t => Array.isArray(t.steps) && t.steps.every(s => knownRoutes.has(s.route)));

  watch(active, v => write(ACTIVE_KEY, v), { deep: true });
  watch(customTemplates, v => write(CUSTOM_KEY, v), { deep: true });
  watch(history, v => write(HISTORY_KEY, v), { deep: true });

  const templates = computed(() => [...BUILTIN_MISSIONS, ...customTemplates.value]);
  const currentStep = computed(() => active.value?.steps[active.value.current] ?? null);
  const doneCount = computed(() => active.value?.steps.filter(s => s.status !== "pending").length ?? 0);
  const progress = computed(() => active.value ? Math.round((doneCount.value / active.value.steps.length) * 100) : 0);
  const isFinished = computed(() => !!active.value && doneCount.value === active.value.steps.length);

  function start(templateId: string, client = ""): MissionStep | null {
    const t = templates.value.find(x => x.id === templateId);
    if (!t || !t.steps.length) return null;
    active.value = {
      templateId: t.id, name: t.name, client: client.trim(),
      startedAt: new Date().toISOString(), current: 0,
      steps: t.steps.map(s => ({ ...s, status: "pending" as StepStatus })),
    };
    return active.value.steps[0];
  }

  /** Marque l'étape courante et renvoie la suivante encore à faire (ou null). */
  function complete(status: Exclude<StepStatus, "pending"> = "done"): MissionStep | null {
    const m = active.value;
    if (!m) return null;
    const step = m.steps[m.current];
    if (step) { step.status = status; step.finishedAt = new Date().toISOString(); }
    const next = m.steps.findIndex(s => s.status === "pending");
    if (next < 0) { m.finishedAt = m.finishedAt ?? new Date().toISOString(); return null; }
    m.current = next;
    return m.steps[next];
  }

  function goTo(index: number) {
    if (active.value && index >= 0 && index < active.value.steps.length) active.value.current = index;
  }

  function reopen(index: number) {
    const s = active.value?.steps[index];
    if (!s || !active.value) return;
    s.status = "pending"; s.finishedAt = undefined;
    active.value.finishedAt = undefined;
    active.value.current = index;
  }

  function setNote(index: number, note: string) {
    const s = active.value?.steps[index];
    if (s) s.note = note;
  }

  /** Termine (ou abandonne) la mission et l'archive dans l'historique. */
  function stop() {
    if (!active.value) return;
    const m = { ...active.value, finishedAt: active.value.finishedAt ?? new Date().toISOString() };
    history.value = [m, ...history.value].slice(0, HISTORY_MAX);
    active.value = null;
  }

  function saveTemplate(name: string, description: string, routes: string[], id?: string): MissionTemplate | null {
    const steps = routes.filter(r => knownRoutes.has(r)).map(route => ({ route, hint: "" }));
    if (!name.trim() || !steps.length) return null;
    const t: MissionTemplate = { id: id ?? `custom-${Date.now().toString(36)}`, name: name.trim(), description: description.trim(), steps, custom: true };
    const i = customTemplates.value.findIndex(x => x.id === t.id);
    if (i >= 0) customTemplates.value.splice(i, 1, t); else customTemplates.value.push(t);
    return t;
  }

  function deleteTemplate(id: string) {
    customTemplates.value = customTemplates.value.filter(t => t.id !== id);
  }

  return {
    active, templates, customTemplates, history,
    currentStep, doneCount, progress, isFinished,
    start, complete, goTo, reopen, setNote, stop, saveTemplate, deleteTemplate,
  };
});

const STATUS_LABEL: Record<StepStatus, string> = { pending: "à faire", done: "fait", skipped: "passé" };

function fmt(iso?: string) {
  return iso ? new Date(iso).toLocaleString("fr-FR", { dateStyle: "short", timeStyle: "short" }) : "—";
}

/** Rapport texte d'une mission (copie ou export .txt). */
export function missionReport(m: ActiveMission): string[] {
  const lines = [`RAPPORT D'INTERVENTION — ${m.name}`];
  if (m.client) lines.push(`Client / poste : ${m.client}`);
  lines.push(`Début : ${fmt(m.startedAt)}    Fin : ${fmt(m.finishedAt)}`, "");
  m.steps.forEach((s, i) => {
    lines.push(`${i + 1}. [${STATUS_LABEL[s.status]}] ${toolLabel(s.route)}${s.finishedAt ? ` — ${fmt(s.finishedAt)}` : ""}`);
    if (s.note?.trim()) lines.push(`   Note : ${s.note.trim()}`);
  });
  const done = m.steps.filter(s => s.status === "done").length;
  lines.push("", `${done} étape(s) faite(s) sur ${m.steps.length}.`, "Rapport généré par NiTriTe.");
  return lines;
}
