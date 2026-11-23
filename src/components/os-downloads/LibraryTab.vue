<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) — bibliothèque : ISO et images locales
// inspectées (éditions WIM, démarrage BIOS/UEFI) et empreinte comparée au catalogue.
import { computed, onMounted, ref } from "vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import NModal from "@/components/ui/NModal.vue";
import { FilePlus, FolderPlus, Search, ShieldCheck } from "lucide-vue-next";
import { useNotificationStore } from "@/stores/notifications";
import { api, displayBytes, errorText, normalize, type MediaFile } from "./useIsoCatalog";

const notifications = useNotificationStore();
const files = ref<MediaFile[]>([]);
const query = ref("");
const duplicates = ref(false);
const busy = ref(false);
const status = ref("");
const current = ref<MediaFile | null>(null);

const STATE_LABELS: Record<MediaFile["state"], string> = { ready: "Présent", absent: "Support absent", changed: "À réindexer" };

const shown = computed(() => {
  const q = normalize(query.value.trim());
  const rows = files.value.filter(f => (!duplicates.value || f.duplicates > 0) && normalize([f.name, f.path, ...f.known].join(" ")).includes(q));
  return duplicates.value ? [...rows].sort((a, b) => a.sha256.localeCompare(b.sha256) || a.name.localeCompare(b.name)) : rows;
});

async function refresh() {
  try { files.value = (await api<{ files: MediaFile[] }>("library-list")).files; }
  catch (error) { status.value = errorText(error); }
}
onMounted(refresh);

async function add(directory: boolean) {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const path = await open(directory ? { directory: true, title: "Dossier de médias à indexer" }
    : { title: "Image à indexer", filters: [{ name: "Images", extensions: ["iso", "img", "wim", "esd", "dmg", "vhdx", "qcow2", "ova", "zip", "7z", "rar", "gz", "xz", "bz2", "zst"] }] });
  if (typeof path !== "string") return;
  busy.value = true;
  status.value = "Lecture des fichiers et calcul des empreintes…";
  try {
    const result = await api<{ indexed: number; skipped: number; limited: boolean; files: MediaFile[] }>("library-add", { path }, 3_600_000);
    files.value = result.files;
    status.value = `${result.indexed} fichier(s) indexé(s) · ${result.skipped} ignoré(s).${result.limited ? " Relance une analyse pour continuer." : ""}`;
  } catch (error) {
    status.value = errorText(error);
  } finally {
    busy.value = false;
  }
}

async function remove(file: MediaFile) {
  try { files.value = (await api<{ files: MediaFile[] }>("library-remove", { id: file.id })).files; }
  catch (error) { notifications.error("Bibliothèque", errorText(error)); }
}

function inspect(file: MediaFile) {
  current.value = file;
  refresh();
}
defineExpose({ inspect });
</script>

<template>
  <div class="iso-stack">
    <div class="iso-row">
      <NButton size="sm" :loading="busy" @click="add(false)"><FilePlus :size="14" /> Ajouter un fichier</NButton>
      <NButton size="sm" variant="secondary" :disabled="busy" @click="add(true)"><FolderPlus :size="14" /> Ajouter un dossier</NButton>
      <div class="iso-row iso-field--grow" style="gap: 6px">
        <Search :size="14" style="color: var(--text-muted)" />
        <input v-model="query" class="iso-field iso-field--grow" placeholder="Rechercher dans la bibliothèque…" />
      </div>
      <label class="iso-row iso-small"><input v-model="duplicates" type="checkbox" /> Doublons seulement</label>
    </div>
    <p v-if="status" class="iso-muted">{{ status }}</p>

    <div class="iso-list">
      <p v-if="!shown.length" class="iso-small">{{ query || duplicates ? "Aucun fichier correspondant." : "Ajoute une ISO, une image disque ou un dossier de médias." }}</p>
      <div v-for="file in shown" :key="file.id" class="iso-line">
        <div class="iso-line-main">
          <div class="iso-row">
            <strong>{{ file.name }}</strong>
            <NBadge :variant="file.state === 'ready' ? 'success' : 'warning'">{{ STATE_LABELS[file.state] }}</NBadge>
            <NBadge v-if="file.known.length" variant="success"><ShieldCheck :size="11" /> Empreinte vérifiée</NBadge>
            <NBadge v-else variant="neutral">Empreinte inconnue du catalogue</NBadge>
            <NBadge v-if="file.windows.length" variant="info">{{ file.windows.length }} édition(s) Windows</NBadge>
          </div>
          <span class="iso-muted">{{ [file.format, displayBytes(file.size), file.boot.join(" / "), file.duplicates ? `${file.duplicates} autre(s) copie(s)` : ""].filter(Boolean).join(" · ") }}</span>
          <span class="iso-muted">{{ file.path }}</span>
        </div>
        <div class="iso-row">
          <NButton size="sm" variant="secondary" @click="current = file">Inspecter</NButton>
          <NButton size="sm" variant="ghost" @click="remove(file)">Retirer</NButton>
        </div>
      </div>
    </div>

    <NModal :open="!!current" :title="current?.name" width="720px" @close="current = null">
      <div v-if="current" class="iso-stack">
        <p class="iso-small">{{ [current.format, displayBytes(current.size), current.boot.length ? `${current.boot.join(" / ")} · Démarrage détecté` : ""].filter(Boolean).join(" · ") }}</p>
        <code class="iso-hash">SHA-256 {{ current.sha256 }}</code>
        <p class="iso-muted">{{ current.path }}</p>
        <p v-if="current.state !== 'ready'" class="iso-muted">Informations du dernier contrôle. Réindexe le fichier pour les actualiser.</p>
        <p v-if="current.known.length" class="iso-small iso-ok">SHA-256 conforme au catalogue · {{ current.known.join(", ") }}</p>
        <p v-else class="iso-small">Cette empreinte ne correspond à aucune source publiée du catalogue.</p>
        <p v-if="current.contents.length" class="iso-small">{{ current.contents.join(" · ") }}</p>
        <p v-if="current.detail" class="iso-muted">{{ current.detail }}</p>
        <template v-if="current.windows.length">
          <span class="iso-section-title">Éditions Windows</span>
          <table class="iso-table">
            <thead><tr><th>Index</th><th>Édition</th><th>Architecture</th><th>Langues</th></tr></thead>
            <tbody>
              <tr v-for="w in current.windows" :key="w.index">
                <td>{{ w.index }}</td><td>{{ w.label || w.edition }}</td><td>{{ w.architecture }}</td><td>{{ w.languages.join(", ") }}</td>
              </tr>
            </tbody>
          </table>
        </template>
      </div>
    </NModal>
  </div>
</template>
