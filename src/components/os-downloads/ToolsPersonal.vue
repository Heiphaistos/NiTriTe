<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) — « Mon catalogue » (liens personnels HTTPS,
// adresses du réseau local refusées) et révisions de catalogue validées avant application.
import { onMounted, ref } from "vue";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import { BookPlus, GitCompare } from "lucide-vue-next";
import { useNotificationStore } from "@/stores/notifications";
import { api, errorText, useIsoCatalog, type IsoItem } from "./useIsoCatalog";

const notifications = useNotificationStore();
const { items } = useIsoCatalog();

interface Personal { id: string; name: string; notes: string; sha256: string; links: string[]; filename: string; path: string }
interface Status { revision: string; canRollback: boolean; personal: Personal[]; items?: IsoItem[] }
interface Change { name: string; item: string; added?: boolean; removed?: boolean; hosts: string[] }

const status = ref<Status | null>(null);
const form = ref({ id: "", name: "", links: "", sha256: "", notes: "", path: "" });
const error = ref("");
const revision = ref({ path: "", sha256: "" });
const preview = ref<{ revision: string; sha256: string; changes: Change[]; pinned: boolean } | null>(null);

function apply(result: Status) {
  status.value = result;
  if (result.items) items.value = result.items;
}

async function call(action: string, payload: Record<string, unknown> = {}) {
  error.value = "";
  try {
    apply(await api<Status>(action, payload));
    return true;
  } catch (e) {
    error.value = errorText(e);
    return false;
  }
}
onMounted(() => call("catalog-status"));

function edit(row: Personal) {
  form.value = { id: row.id, name: row.name, links: row.links.join("\n"), sha256: row.sha256, notes: row.notes, path: "" };
}

async function save() {
  const links = form.value.links.split(/\r?\n/).map(l => l.trim()).filter(Boolean);
  if (await call("personal-save", { ...form.value, id: form.value.id || undefined, links })) {
    notifications.success("Mon catalogue", "Fiche enregistrée.");
    form.value = { id: "", name: "", links: "", sha256: "", notes: "", path: "" };
  }
}

async function pickJson(): Promise<string> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const path = await open({ filters: [{ name: "JSON", extensions: ["json"] }] });
  return typeof path === "string" ? path : "";
}

async function pickLocal() {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const path = await open({ title: "Fichier local de référence" });
  if (typeof path === "string") form.value.path = path;
}

async function exportPersonal() {
  const { save: saveDialog } = await import("@tauri-apps/plugin-dialog");
  const path = await saveDialog({ defaultPath: "mon-catalogue.json", filters: [{ name: "JSON", extensions: ["json"] }] });
  if (!path) return;
  try { await api("personal-export", { path }); notifications.success("Mon catalogue", "Fichier de partage exporté."); }
  catch (e) { error.value = errorText(e); }
}

async function importPersonal() {
  const path = await pickJson();
  if (path && await call("personal-import", { path })) notifications.success("Mon catalogue", "Fiches importées.");
}

async function previewRevision() {
  error.value = "";
  preview.value = null;
  try { preview.value = await api("catalog-preview", { ...revision.value }); }
  catch (e) { error.value = errorText(e); }
}

async function applyRevision(rollback: boolean) {
  if (await call(rollback ? "catalog-rollback" : "catalog-apply")) {
    preview.value = null;
    notifications.success("Catalogue", rollback ? "Révision précédente rétablie." : "Révision appliquée.");
  }
}
</script>

<template>
  <div class="iso-stack">
    <p v-if="error" class="iso-error">{{ error }}</p>
    <NCard>
      <div class="iso-stack">
        <span class="iso-section-title"><BookPlus :size="16" /> Mon catalogue</span>
        <p class="iso-small">Ajoute tes propres liens (HTTPS uniquement, adresses du réseau local refusées). Ils sont signalés « Communauté » dans le catalogue.</p>
        <input v-model="form.name" class="iso-field" maxlength="100" placeholder="Nom de la fiche" />
        <textarea v-model="form.links" class="iso-field" rows="3" placeholder="Liens de téléchargement, un par ligne (12 au plus)" />
        <input v-model="form.sha256" class="iso-field" maxlength="64" placeholder="Empreinte SHA-256 (facultatif)" />
        <input v-model="form.notes" class="iso-field" maxlength="400" placeholder="Notes (facultatif)" />
        <div class="iso-row">
          <input v-model="form.path" class="iso-field iso-field--grow" placeholder="Fichier local de référence (facultatif)" readonly />
          <NButton size="sm" variant="ghost" @click="pickLocal">Parcourir</NButton>
        </div>
        <div class="iso-row">
          <NButton size="sm" :disabled="!form.name.trim()" @click="save">{{ form.id ? "Mettre à jour" : "Ajouter" }}</NButton>
          <NButton v-if="form.id" size="sm" variant="ghost" @click="form = { id: '', name: '', links: '', sha256: '', notes: '', path: '' }">Annuler</NButton>
          <NButton size="sm" variant="secondary" @click="importPersonal">Importer</NButton>
          <NButton size="sm" variant="secondary" :disabled="!status?.personal.length" @click="exportPersonal">Exporter</NButton>
        </div>
        <div class="iso-list">
          <div v-for="row in status?.personal ?? []" :key="row.id" class="iso-line">
            <div class="iso-line-main">
              <strong>{{ row.name }}</strong>
              <span class="iso-muted">{{ row.links.length }} lien(s){{ row.filename ? ` · ${row.filename}` : "" }}</span>
            </div>
            <NButton size="sm" variant="ghost" @click="edit(row)">Modifier</NButton>
            <NButton size="sm" variant="ghost" @click="call('personal-delete', { id: row.id })">Supprimer</NButton>
          </div>
        </div>
      </div>
    </NCard>

    <NCard>
      <div class="iso-stack">
        <span class="iso-section-title"><GitCompare :size="16" /> Révision du catalogue</span>
        <p class="iso-small">Révision active : {{ status?.revision }}. Charge un fichier de révision TokaISO pour mettre à jour les liens sans attendre une nouvelle version de NiTriTe.</p>
        <div class="iso-row">
          <input v-model="revision.path" class="iso-field iso-field--grow" placeholder="Fichier de révision (.json)" readonly />
          <NButton size="sm" variant="ghost" @click="pickJson().then(p => p && (revision.path = p))">Parcourir</NButton>
        </div>
        <input v-model="revision.sha256" class="iso-field" maxlength="64" placeholder="Empreinte SHA-256 publiée de la révision (recommandé)" />
        <div class="iso-row">
          <NButton size="sm" :disabled="!revision.path" @click="previewRevision">Vérifier</NButton>
          <NButton size="sm" variant="secondary" :disabled="!status?.canRollback" @click="applyRevision(true)">Revenir à la révision précédente</NButton>
        </div>
        <div v-if="preview" class="iso-stack">
          <span class="iso-small">Révision {{ preview.revision }} · {{ preview.changes.length }} changement(s){{ preview.pinned ? " · empreinte confirmée" : " · empreinte non fournie" }}</span>
          <ul class="iso-small">
            <li v-for="c in preview.changes.slice(0, 50)" :key="c.item">
              {{ c.added ? "Ajout" : c.removed ? "Retrait" : "Modification" }} · {{ c.name }}<template v-if="c.hosts.length"> · {{ c.hosts.join(", ") }}</template>
            </li>
          </ul>
          <div><NButton size="sm" @click="applyRevision(false)">Appliquer la révision</NButton></div>
        </div>
      </div>
    </NCard>
  </div>
</template>
