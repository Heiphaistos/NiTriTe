<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) — trousse : plusieurs fichiers dans un dossier
// commun avec son manifeste (NiTriTe-ISO-manifeste.json), à partir de modèles prêts.
import { computed, ref } from "vue";
import NModal from "@/components/ui/NModal.vue";
import NButton from "@/components/ui/NButton.vue";
import { useNotificationStore } from "@/stores/notifications";
import { api, confirmCommunity, displayBytes, errorText, normalize, useIsoCatalog, KIND_LABELS, type IsoItem, type Snapshot } from "./useIsoCatalog";

const emit = defineEmits<{ queued: [] }>();
const notifications = useNotificationStore();
const { items, snapshot } = useIsoCatalog();

interface Entry { item: IsoItem; source: number; included: boolean }
const open = ref(false);
const name = ref("Ma trousse");
const search = ref("");
const selection = ref<Entry[]>([]);
const busy = ref(false);

const PRESETS: { name: string; description: string; files: RegExp[] }[] = [
  { name: "Réinstaller ce PC", description: "Un média Windows, un outil USB et les applications nécessaires après la réinstallation.", files: [/^Windows 11 26H2/, /^Rufus · édition portable/, /^DiskGenius/, /^7-Zip/, /^Mozilla Firefox/] },
  { name: "Nouveau PC", description: "Un système, une suite Office et les outils essentiels.", files: [/Windows 11 26H2/, /Microsoft 365 · kit tout-en-un/, /^Mozilla Firefox/, /^7-Zip/] },
  { name: "Clé de secours", description: "Clé multiboot, test mémoire et intervention sur un disque.", files: [/^Ventoy/, /^SystemRescue/, /^GParted Live/, /^Clonezilla Live/, /^Memtest86\+/] },
  { name: "Labo de virtualisation", description: "Un hyperviseur, un serveur d’évaluation et une image cliente.", files: [/^Proxmox VE/, /^Windows Server 2025 · Évaluation/, /^Ubuntu 26\.04 LTS · Desktop/] },
  { name: "Labo réseau", description: "Des pare-feu et routeurs à essayer en machines virtuelles.", files: [/^OPNsense/, /^IPFire/, /^OpenWrt/] },
  { name: "NAS et sauvegarde", description: "Un système NAS et un serveur de sauvegarde.", files: [/^TrueNAS Community/, /^Proxmox Backup Server/] },
  { name: "Console portable", description: "Une image adaptée à ton modèle et un outil pour préparer la clé.", files: [/^Bazzite Deck/, /^CachyOS Handheld/, /^Nobara.*Steam Handheld/, /^Rufus · édition portable/] },
  { name: "Réparation d’un PC", description: "Clonage, récupération de fichiers et diagnostic Windows.", files: [/^Rescuezilla/, /^TestDisk et PhotoRec/, /^Microsoft Sysinternals/, /^CrystalDiskInfo/] },
  { name: "Labo réseau et NAS", description: "Pare-feu, stockage et analyse du réseau.", files: [/^OPNsense/, /^TrueNAS Community/, /^Wireshark/, /^Angry IP Scanner/] },
  { name: "Mini-serveur maison", description: "Domotique, DNS et synchronisation de fichiers.", files: [/^Raspberry Pi OS · Lite/, /^Home Assistant OS · Raspberry Pi/, /^AdGuard Home/, /^Syncthing/] },
  { name: "Mon premier labo réseau", description: "OPNsense et un client Linux sur un réseau isolé.", files: [/^OPNsense/, /^Debian 13 · GenericCloud/, /^Angry IP Scanner/] },
  { name: "Récupérer un disque", description: "Un média de secours et des outils de récupération.", files: [/^Hiren/, /^DiskGenius/, /^DMDE/, /^TestDisk et PhotoRec/] },
  { name: "Création et montage", description: "Montage, son et 3D.", files: [/^Kdenlive/, /^Audacity/, /^Blender/] },
  { name: "Poste de développement", description: "Code et bases de données.", files: [/^Git for Windows/, /^DBeaver/, /^Notepad\+\+/] },
  { name: "Clé de technicien", description: "Médias de secours, test mémoire et clé multiboot.", files: [/^Hiren/, /^WinPE Sergei/, /^Rescuezilla · ISO/, /^Memtest86\+/, /^Ventoy/] },
  { name: "Je change de SSD", description: "Migration du disque et vérification du résultat.", files: [/^AOMEI Cloner/, /^Rescuezilla · ISO/, /^CrystalDiskInfo/, /^DiskGenius/] },
  { name: "PC gaming neuf", description: "Lanceurs et outils de diagnostic.", files: [/^Steam · Windows/, /^Epic Games/, /^Xbox ·/, /^HWiNFO/, /^OBS Studio/] },
];

const downloadable = (item: IsoItem, index: number) => item.sources[index]?.kind !== "portal";

function add(item: IsoItem, source?: number) {
  if (selection.value.length >= 24 || selection.value.some(e => e.item.id === item.id)) return;
  const direct = item.sources.map((s, i) => ({ s, i })).filter(x => x.s.kind !== "portal");
  const preferred = source ?? (direct.find(x => /^ISO(?:\.|$)/.test(x.s.format || "")) ?? direct[0])?.i ?? 0;
  selection.value.push({ item, source: preferred, included: true });
}

function show(preset?: (typeof PRESETS)[number], item?: IsoItem, source?: number) {
  if (preset) {
    selection.value = [];
    name.value = preset.name;
    for (const pattern of preset.files) {
      const found = items.value.find(c => pattern.test(c.name) && c.sources.length);
      if (found) add(found);
    }
  }
  if (item) add(item, source);
  open.value = true;
}
defineExpose({ show });

const options = computed(() => {
  const q = normalize(search.value.trim());
  if (!q) return [];
  return items.value.filter(i => i.sources.some(s => s.kind !== "portal") && !selection.value.some(e => e.item.id === i.id)
    && normalize(`${i.name} ${i.family}`).includes(q)).slice(0, 12);
});

const chosen = computed(() => selection.value.filter(e => e.included && downloadable(e.item, e.source)));
const total = computed(() => chosen.value.reduce((sum, e) => sum + (e.item.sources[e.source].size || 0), 0));

async function queue() {
  if (!name.value.trim()) { notifications.warning("Trousse", "Donne un nom à ta trousse."); return; }
  if (!(await confirmCommunity(chosen.value.map(e => e.item.sources[e.source])))) return;
  busy.value = true;
  try {
    snapshot.value = await api<Snapshot>("batch", { name: name.value, items: chosen.value.map(e => ({ item: e.item.id, source: e.source })) });
    notifications.success("Trousse ajoutée", `${chosen.value.length} fichier(s) dans « ${name.value} »`);
    open.value = false;
    emit("queued");
  } catch (error) {
    notifications.error("Trousse", errorText(error));
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <NModal :open="open" title="Trousse de téléchargement" width="760px" @close="open = false">
    <div class="iso-stack">
      <div class="iso-chips">
        <button v-for="p in PRESETS" :key="p.name" class="iso-chip" :title="p.description" @click="show(p)">{{ p.name }}</button>
      </div>
      <label class="iso-label">Nom de la trousse (dossier créé dans le dossier de téléchargement)
        <input v-model="name" class="iso-field" maxlength="48" />
      </label>
      <div class="iso-list">
        <p v-if="!selection.length" class="iso-small">Choisis un modèle ou ajoute un fichier depuis la recherche ci-dessous.</p>
        <div v-for="(entry, idx) in selection" :key="entry.item.id" class="iso-line">
          <label class="iso-row" style="flex: 1">
            <input v-model="entry.included" type="checkbox" />
            <span class="iso-small"><strong>{{ entry.item.name }}</strong></span>
          </label>
          <select v-model.number="entry.source" class="iso-field" :disabled="!entry.included" style="max-width: 320px">
            <option v-for="(s, i) in entry.item.sources" :key="i" :value="i" :disabled="s.kind === 'portal'">
              {{ [s.label, s.format, s.arch, KIND_LABELS[s.kind]].filter(Boolean).join(" · ") }}
            </option>
          </select>
          <NButton size="sm" variant="ghost" @click="selection.splice(idx, 1)">Retirer</NButton>
        </div>
      </div>
      <input v-model="search" class="iso-field" placeholder="Ajouter un fichier du catalogue…" />
      <div v-if="options.length" class="iso-chips">
        <button v-for="o in options" :key="o.id" class="iso-chip" :disabled="selection.length >= 24" @click="add(o); search = ''">+ {{ o.name }}</button>
      </div>
    </div>
    <template #footer>
      <span class="iso-muted">{{ chosen.length }} fichier(s){{ total ? ` · ≈ ${displayBytes(total)}` : "" }}</span>
      <NButton :disabled="!chosen.length" :loading="busy" @click="queue">Télécharger la trousse</NButton>
    </template>
  </NModal>
</template>
