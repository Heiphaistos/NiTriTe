<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) — fiche détaillée d'une entrée du catalogue.
import { ref, watch } from "vue";
import NModal from "@/components/ui/NModal.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import { Download, ExternalLink, RefreshCw, ShieldCheck, Cpu, Library, PackagePlus, AlertTriangle } from "lucide-vue-next";
import { useNotificationStore } from "@/stores/notifications";
import {
  api, displayBytes, errorText, familyLabel, openPortal, useIsoCatalog, KIND_LABELS, KIND_VARIANTS,
  type IsoItem, type IsoSource, type MediaFile,
} from "./useIsoCatalog";

const props = defineProps<{ item: IsoItem | null }>();
const emit = defineEmits<{ close: []; bundle: [item: IsoItem, source: number]; inspect: [file: MediaFile] }>();

const notifications = useNotificationStore();
const { queueDownload } = useIsoCatalog();

interface Check { ok: boolean; status: string; message: string; detail?: string; checked?: string; code?: number }
const checks = ref<(Check | null)[]>([]);
const reusable = ref<number[]>([]);
const busy = ref<Set<number>>(new Set());
const compat = ref<Record<number, string[]>>({});

const installLabels: Record<string, string> = {
  boot: "Média amorçable", online: "Internet nécessaire pour l’installation", builder: "Créateur de média", local: "Fichier à emporter",
};

watch(() => props.item, async item => {
  checks.value = [];
  reusable.value = [];
  compat.value = {};
  if (!item) return;
  try {
    checks.value = (await api<{ checks: (Check | null)[] }>("source-item", { item: item.id })).checks;
    if (item.sources.some(s => s.sha256)) reusable.value = (await api<{ sources: number[] }>("library-reusable", { item: item.id })).sources;
  } catch (error) {
    notifications.error("Fiche", errorText(error));
  }
});

function summary(check: Check | null): string {
  if (!check) return "";
  const text = (check.message || "").replace("{code}", String(check.code ?? "?")).replace("{detail}", check.detail ?? "");
  const when = check.checked ? new Date(check.checked).toLocaleString("fr-FR") : "";
  return [text, when && `contrôlé le ${when}`].filter(Boolean).join(" · ");
}

async function run(index: number, task: () => Promise<void>) {
  busy.value = new Set(busy.value).add(index);
  try { await task(); }
  catch (error) { notifications.error("Téléchargement OS", errorText(error)); }
  finally {
    const next = new Set(busy.value);
    next.delete(index);
    busy.value = next;
  }
}

const download = (index: number) => run(index, async () => {
  if (props.item && await queueDownload(props.item, index)) notifications.success("Ajouté à la file", props.item.name);
});

const recheck = (index: number) => run(index, async () => {
  const result = await api<Check>("source-test", { item: props.item?.id, source: index }, 30_000);
  checks.value = checks.value.map((c, i) => (i === index ? result : c));
  if (checks.value.length <= index) checks.value[index] = result;
});

const compatibility = (index: number) => run(index, async () => {
  const r = await api<{ checks: Record<string, boolean | null>; requirements: { url?: string } }>("deployment-compatibility", { item: props.item?.id, source: index });
  const label = (v: boolean | null) => (v === null ? "non déterminé" : v ? "compatible" : "incompatible");
  compat.value = { ...compat.value, [index]: [
    `Architecture : ${label(r.checks.architecture)}`, `Démarrage (BIOS/UEFI) : ${label(r.checks.firmware)}`,
    `Mémoire : ${label(r.checks.memory)}`, `Espace disque : ${label(r.checks.space)}`,
  ] };
});

const reuse = (index: number) => run(index, async () => {
  const data = await api<{ file: MediaFile }>("library-reuse", { item: props.item?.id, source: index }, 3_600_000);
  emit("inspect", data.file);
});

const isMedia = (s: IsoSource) => /^(ISO|IMG)(?:[.\s]|$)/i.test(s.format || "");
</script>

<template>
  <NModal :open="!!item" :title="item?.name" width="820px" @close="emit('close')">
    <div v-if="item" class="iso-stack">
      <p class="iso-small">{{ item.desc }}</p>
      <div class="iso-row">
        <NBadge variant="neutral">{{ familyLabel(item.family) }}</NBadge>
        <NBadge v-if="item.install && installLabels[item.install]" variant="info">{{ installLabels[item.install] }}</NBadge>
        <NBadge v-if="item.personal" variant="accent">Mon catalogue</NBadge>
      </div>
      <details v-if="item.vm" class="iso-small">
        <summary>Premier démarrage</summary>
        <p>{{ item.vm.ram }} Go de RAM · {{ item.vm.cpu }} vCPU · {{ item.vm.disk }} Go d’espace disque à prévoir</p>
        <ol><li v-for="step in item.vm.steps" :key="step">{{ step }}</li></ol>
      </details>
      <p v-if="!item.sources.length" class="iso-small">Aucun lien direct n’a pu être confirmé lors de la vérification.</p>

      <div v-for="(source, index) in item.sources" :key="index" class="iso-source" :class="{ 'iso-source--community': source.kind === 'community' }">
        <div class="iso-row">
          <strong>{{ source.label }}</strong>
          <NBadge :variant="KIND_VARIANTS[source.kind]">{{ KIND_LABELS[source.kind] }}</NBadge>
          <NBadge v-if="source.sha256" variant="success"><ShieldCheck :size="11" /> SHA-256</NBadge>
          <span class="iso-muted">{{ [source.format, source.arch, displayBytes(source.size)].filter(Boolean).join(" · ") }}</span>
        </div>
        <div v-if="source.kind === 'community'" class="iso-banner">
          <AlertTriangle :size="16" style="color: var(--warning); flex-shrink: 0" />
          <span>Origine non officielle : l’image peut avoir été modifiée. Vérifie son empreinte et respecte la licence de l’éditeur.</span>
        </div>
        <span v-if="source.filename && source.kind !== 'portal'" class="iso-muted">{{ source.filename }}</span>
        <span v-if="source.note" class="iso-muted">{{ source.note }}</span>
        <span v-if="source.sample" class="iso-muted">Format reconnu · {{ source.sample === "PE" ? "Exécutable Windows (PE)" : source.sample }}</span>
        <span class="iso-muted">{{ [source.language, source.verified && `Lien contrôlé le ${source.verified}`].filter(Boolean).join(" · ") }}</span>
        <span v-if="source.mirror_count" class="iso-muted">Bascule automatique · {{ source.mirror_count }} miroir(s) de même empreinte</span>
        <code v-if="source.sha256" class="iso-hash">SHA-256 {{ source.sha256 }}</code>
        <span v-if="checks[index]" class="iso-muted" :class="checks[index]?.ok ? 'iso-ok' : 'iso-warn'">{{ summary(checks[index]) }}</span>
        <ul v-if="compat[index]" class="iso-small"><li v-for="line in compat[index]" :key="line">{{ line }}</li></ul>
        <div class="iso-row">
          <NButton v-if="source.kind === 'portal'" size="sm" variant="secondary" @click="openPortal(source.url)">
            <ExternalLink :size="14" /> Ouvrir le portail
          </NButton>
          <NButton v-else size="sm" :loading="busy.has(index)" @click="download(index)">
            <Download :size="14" /> Télécharger
          </NButton>
          <NButton v-if="source.kind !== 'portal'" size="sm" variant="ghost" @click="emit('bundle', item, index)">
            <PackagePlus :size="14" /> Ajouter à la trousse
          </NButton>
          <NButton v-if="reusable.includes(index)" size="sm" variant="ghost" :disabled="busy.has(index)" @click="reuse(index)">
            <Library :size="14" /> Utiliser le fichier de la bibliothèque
          </NButton>
          <NButton size="sm" variant="ghost" :disabled="busy.has(index)" @click="recheck(index)">
            <RefreshCw :size="14" /> Revérifier
          </NButton>
          <NButton v-if="isMedia(source)" size="sm" variant="ghost" :disabled="busy.has(index)" @click="compatibility(index)">
            <Cpu :size="14" /> Compatibilité avec ce PC
          </NButton>
        </div>
      </div>
    </div>
  </NModal>
</template>
