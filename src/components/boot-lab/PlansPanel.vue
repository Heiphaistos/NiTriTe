<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : plans de mini-serveurs (extras.js #plans-dialog) et modèles de labo
// (deployment.js renderServerTemplates, labs.json).
import { onMounted, ref } from "vue";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import { ClipboardList, Plus, Trash2, Pencil, FlaskConical, ExternalLink } from "lucide-vue-next";
import { useNotificationStore } from "@/stores/notifications";
import { call, openLink } from "./useBootLab";

interface PlanMachine { name: string; host: string; role: string; item: string; ports: number[] }
interface Plan { id: string; name: string; machines: PlanMachine[]; updated: string }
interface ServerLab { key: string; name: string; role: string; ports: number[]; items: string[]; ram: number; cpu: number; disk: number; steps: string[] }
interface VmLab { name: string; description: string; items: string[]; steps: string[]; reference?: string }
interface Labs { vm: Record<string, VmLab>; servers: ServerLab[] }
interface Saved { plans: Plan[]; id: string }

const emit = defineEmits<{ test: [target: { host: string; port: number }] }>();
const notify = useNotificationStore();
const roles: Record<string, string> = { home: "Domotique", storage: "Partage de fichiers", web: "Labo web", network: "Réseau", other: "Autre" };
const plans = ref<Plan[]>([]);
const labs = ref<Labs | null>(null);
const editing = ref<{ id?: string; name: string; machines: (PlanMachine & { portsText: string })[] } | null>(null);
const saving = ref(false);

const quick: [string, string, string, number[]][] = [
  ["home", "Home Assistant", "vm-home-assistant", [8123]],
  ["storage", "Serveur de fichiers", "vm-ucs", [445, 443]],
  ["web", "Ubuntu Server", "vm-ubuntu-osboxes", [80, 443, 22]],
];

function row(m: Partial<PlanMachine> = {}) {
  return { name: m.name ?? "", host: m.host ?? "", role: m.role ?? "other", item: m.item ?? "", ports: m.ports ?? [], portsText: (m.ports ?? []).join(", ") };
}

function edit(plan: { id?: string; name: string; machines: Partial<PlanMachine>[] }) {
  editing.value = { id: plan.id, name: plan.name, machines: plan.machines.map(row) };
}

async function load() {
  plans.value = (await call<Plan[]>("boot_lab_plans")) ?? [];
  labs.value = await call<Labs>("boot_lab_labs");
}

async function save() {
  if (!editing.value) return;
  saving.value = true;
  const machines = editing.value.machines.map(({ portsText, ...m }) => ({
    ...m, ports: portsText.split(/[\s,;]+/).filter(Boolean).map(Number),
  }));
  const result = await call<Saved>("boot_lab_plan_save", { id: editing.value.id ?? null, name: editing.value.name, machines });
  saving.value = false;
  if (!result) return;
  plans.value = result.plans;
  editing.value = null;
  notify.success("Projet enregistré.");
}

async function remove(plan: Plan) {
  const { confirm } = await import("@tauri-apps/plugin-dialog");
  if (!(await confirm(`Supprimer le projet « ${plan.name} » ?`, { title: "Plans de mini-serveurs", kind: "warning" }))) return;
  const result = await call<Plan[]>("boot_lab_plan_delete", { id: plan.id });
  if (result) plans.value = result;
}

async function fromLab(key: string) {
  const result = await call<Saved>("boot_lab_lab_plan", { key });
  if (!result) return;
  plans.value = result.plans;
  notify.success("Projet réseau créé.");
}

onMounted(load);
</script>

<template>
  <div class="bl-panel">
    <NCard>
      <div class="bl-panel">
        <div class="bl-title"><ClipboardList :size="16" /> Plans de mini-serveurs</div>
        <div class="bl-actions">
          <NButton variant="secondary" @click="edit({ name: '', machines: [{}] })"><Plus :size="14" /> Nouveau projet</NButton>
          <NButton v-for="[role, name, item, ports] in quick" :key="role" variant="secondary"
            @click="edit({ name: roles[role], machines: [{ name, role, item, ports }] })">{{ roles[role] }}</NButton>
        </div>

        <div v-if="editing" class="bl-panel">
          <div class="bl-fields">
            <label>Nom du projet <input v-model="editing.name" maxlength="70" /></label>
            <NButton size="sm" variant="secondary" :disabled="editing.machines.length >= 16" @click="editing.machines.push(row())">Ajouter une machine</NButton>
          </div>
          <div v-for="(m, i) in editing.machines" :key="i" class="bl-fields">
            <label>Nom <input v-model="m.name" maxlength="70" /></label>
            <label>Adresse IP ou nom <input v-model="m.host" maxlength="253" /></label>
            <label>Rôle
              <select v-model="m.role"><option v-for="(label, key) in roles" :key="key" :value="key">{{ label }}</option></select>
            </label>
            <label>Ports TCP <input v-model="m.portsText" placeholder="80, 443" /></label>
            <NButton size="sm" variant="ghost" @click="editing.machines.splice(i, 1)"><Trash2 :size="14" /></NButton>
          </div>
          <div class="bl-actions">
            <NButton :loading="saving" @click="save">Enregistrer</NButton>
            <NButton variant="ghost" @click="editing = null">Annuler</NButton>
          </div>
        </div>

        <p v-if="!plans.length" class="bl-muted">Aucun projet enregistré.</p>
        <div v-for="plan in plans" :key="plan.id" class="bl-panel">
          <div class="bl-row">
            <div class="bl-row-main"><span class="bl-row-name">{{ plan.name }}</span><span class="bl-muted">{{ plan.machines.length }} machine(s)</span></div>
            <div class="bl-actions">
              <NButton size="sm" variant="secondary" @click="edit(plan)"><Pencil :size="14" /> Modifier</NButton>
              <NButton size="sm" variant="danger" @click="remove(plan)"><Trash2 :size="14" /> Supprimer</NButton>
            </div>
          </div>
          <div v-for="(m, i) in plan.machines" :key="i" class="bl-row" style="margin-left: 16px">
            <div class="bl-row-main">
              <span class="bl-row-name">{{ m.name }}</span>
              <span class="bl-muted">{{ [roles[m.role], m.item, m.host].filter(Boolean).join(" · ") }}</span>
            </div>
            <div v-if="m.host" class="bl-actions">
              <NButton v-for="p in m.ports" :key="p" size="sm" variant="secondary" @click="emit('test', { host: m.host, port: p })">Tester le port {{ p }}</NButton>
            </div>
          </div>
        </div>
      </div>
    </NCard>

    <NCard v-if="labs">
      <div class="bl-panel">
        <div class="bl-title"><FlaskConical :size="16" /> Modèles de labo</div>
        <div v-for="lab in labs.servers" :key="lab.key" class="bl-panel">
          <div class="bl-row">
            <div class="bl-row-main">
              <span class="bl-row-name">{{ lab.name }}</span>
              <span class="bl-muted">{{ lab.ram }} Gio de RAM · {{ lab.cpu }} vCPU · {{ lab.disk }} Gio pour le labo · ports {{ lab.ports.join(", ") }}</span>
            </div>
            <NButton size="sm" variant="secondary" @click="fromLab(lab.key)">Créer le projet réseau</NButton>
          </div>
          <ol class="bl-steps"><li v-for="step in lab.steps" :key="step">{{ step }}</li></ol>
        </div>
        <div class="bl-title">Trousses VM</div>
        <div v-for="(lab, key) in labs.vm" :key="key" class="bl-panel">
          <div class="bl-row-main">
            <span class="bl-row-name">{{ lab.name }}</span>
            <span class="bl-muted">{{ lab.description }}</span>
          </div>
          <ol class="bl-steps"><li v-for="step in lab.steps" :key="step">{{ step }}</li></ol>
          <div v-if="lab.reference" class="bl-actions">
            <NButton size="sm" variant="ghost" @click="openLink(lab.reference)"><ExternalLink :size="14" /> Guide d’installation Gitea</NButton>
          </div>
        </div>
      </div>
    </NCard>
  </div>
</template>
