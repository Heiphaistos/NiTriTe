<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) — « Catalogue complet » : 32 familles,
// recherche, filtres (famille, architecture, type de source, rôle), favoris.
import { computed, ref, watch } from "vue";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import { Search, PackagePlus, ShieldCheck } from "lucide-vue-next";
import { useNotificationStore } from "@/stores/notifications";
import ItemDetail from "./ItemDetail.vue";
import BundleDialog from "./BundleDialog.vue";
import {
  api, displayBytes, errorText, familyLabel, normalize, useIsoCatalog, FAMILIES, KIND_LABELS, KIND_VARIANTS,
  type IsoItem, type MediaFile, type SourceKind, type Snapshot,
} from "./useIsoCatalog";

const emit = defineEmits<{ inspect: [file: MediaFile]; queued: [] }>();
const notifications = useNotificationStore();
const { items, favorites, snapshot } = useIsoCatalog();

const family = ref("all");
const query = ref("");
const arch = ref("all");
const kind = ref("all");
const tag = ref("all");
const role = ref("all");
const visible = ref(30);
const detail = ref<IsoItem | null>(null);
const bundle = ref<InstanceType<typeof BundleDialog> | null>(null);

// Rôles par famille (filtres secondaires de TokaISO).
const GROUPS: Record<string, [string, string][]> = {
  "Linux Gaming": [["pc", "PC gaming"], ["handheld", "Console portable"], ["tv", "PC de salon"], ["recovery", "Récupération"]],
  "Rétrogaming et streaming": [["emulation", "Émulation"], ["stream", "Jeu à distance"]],
  "Serveurs et stockage": [["microsoft", "Windows Server"], ["nas", "NAS"], ["backup", "Sauvegarde"], ["platform", "Plateformes"], ["home", "Mini-serveurs"]],
  "Pare-feu": [["iso", "ISO"], ["image", "Images disque / installateurs"], ["ipam", "Gestion IP"], ["tools", "Outils réseau"]],
  "Secours et diagnostic": [["usb", "Clés et démarrage"], ["disk", "Disques"], ["windows", "Windows"]],
  "Installation d’applications": [["productivity", "Bureautique et utilitaires"], ["media", "Multimédia"], ["backup", "Sauvegarde"]],
  "Archives et systèmes légers": [["legacy", "Archives et 32 bits"], ["light", "Linux léger"]],
  "Live PE et récupération": [["boot", "Trousse / ISO PE"], ["builders", "Créateurs de médias"], ["linux", "Autres médias de secours"]],
  "Pilotes et déploiement": [["models", "Packs par modèle"], ["deploy", "Déploiement"], ["drivers", "Pilotes WinPE"], ["recovery", "Récupération constructeur"]],
  "Sauvegarde et migration": [["image", "Images et sauvegarde"], ["clone", "Clonage et restauration"], ["sync", "Fichiers et synchronisation"]],
  "USB, démarrage et déploiement": [["usb", "Clés et images"], ["deploy", "Déploiement Windows"]],
  "Outils portables": [["system", "Système"], ["network", "Réseau"], ["files", "Fichiers"]],
  "Utilitaires hors ligne": [["browser", "Navigateurs"], ["runtime", "Composants Windows"], ["network", "Pilotes réseau"], ["tools", "Outils"]],
  "Démarrage réseau PXE": [["media", "Images de démarrage"], ["loader", "Chargeurs"]],
  "Images de machines virtuelles": [["ova", "Appliances OVA"], ["disk", "Disques virtuels"]],
  "Sécurité et surveillance": [["live", "ISO"], ["monitor", "Agents et outils"]],
  "Création, développement et IA": [["media", "Création"], ["dev", "Développement"], ["ai", "IA locale"]],
  "Assistance et systèmes à découvrir": [["assist", "Assistance"], ["experiment", "À essayer en VM"]],
  "Conteneurs et outils de labo": [["desktop", "Applications de bureau"], ["cli", "Ligne de commande"]],
  "Serveurs de sauvegarde": [["server", "Serveurs"], ["client", "Clients"], ["recovery", "Récupération"]],
  "Raspberry Pi et cartes ARM": [["rpi", "Raspberry Pi"], ["sbc", "Autres cartes ARM"], ["media", "Multimédia"]],
  "Diagnostic matériel": [["ram", "Mémoire vive"], ["smart", "Disques et SMART"], ["inventory", "Inventaire"]],
};

interface Indexed { item: IsoItem; tags: Set<string>; arches: Set<string>; search: string }

const indexed = computed<Indexed[]>(() => items.value.map(item => {
  const tags = new Set(item.tags ?? []);
  const arches = new Set<string>();
  for (const s of item.sources) {
    const a = s.arch || "";
    if (/arm64|aarch64/i.test(a)) arches.add("ARM64");
    if (/armhf|arm32|armv[67]|arm \(32 bits\)/i.test(a)) arches.add("ARM32");
    if (/x64|amd64|x86[_-]64/i.test(a)) arches.add("x64");
    if (/\bx86\b|i[36]86/i.test(a) && !/x86[_-]64/i.test(a)) arches.add("x86");
    if (/^ISO(?:\.|$)/i.test(s.format || "")) tags.add("iso");
    if (/^(?:ISO|IMG)(?:\.|$)/i.test(s.format || "") && (s.size ?? 0) > 0 && (s.size ?? 0) <= 1073741824) tags.add("small");
    if (/\bportable\b/i.test(s.label)) tags.add("portable");
  }
  const text = `${item.name} ${item.desc} ${item.family} ${item.sources.map(s => [s.label, s.filename, s.format, s.arch, s.language].join(" ")).join(" ")}`;
  return { item, tags, arches, search: normalize(text) };
}));

const counts = computed(() => {
  const map = new Map<string, number>();
  for (const { item } of indexed.value) {
    map.set(item.family, (map.get(item.family) ?? 0) + 1);
    if (item.section && item.section !== item.family) map.set(item.section, (map.get(item.section) ?? 0) + 1);
  }
  return map;
});
const families = computed(() => FAMILIES.filter(([key]) => counts.value.get(key)));

const filtered = computed(() => {
  const q = normalize(query.value.trim());
  return indexed.value.filter(({ item, tags, arches, search }) => {
    if (family.value === "favorites" ? !favorites.value.has(item.id) : family.value !== "all" && item.family !== family.value && item.section !== family.value) return false;
    if (q && !search.includes(q)) return false;
    if (arch.value !== "all" && !arches.has(arch.value)) return false;
    if (tag.value !== "all" && !tags.has(tag.value)) return false;
    if (role.value !== "all") {
      const value = item.section === family.value ? item.section_group : item.group;
      if (value !== role.value) return false;
    }
    if (kind.value === "unavailable") return !item.sources.some(s => s.kind !== "portal");
    return kind.value === "all" || item.sources.some(s => s.kind === kind.value);
  }).map(x => x.item);
});

watch([family, query, arch, kind, tag], () => { visible.value = 30; });
watch(family, () => { role.value = "all"; });

function kinds(item: IsoItem): SourceKind[] {
  return [...new Set(item.sources.map(s => s.kind))];
}

function footer(item: IsoItem): string {
  const count = item.sources.filter(s => s.kind !== "portal").length;
  if (!count) return item.sources.length ? "Accès éditeur" : "Indisponible";
  const sizes = item.sources.map(s => s.size ?? 0).filter(Boolean);
  const size = sizes.length ? ` · ${displayBytes(Math.min(...sizes))}` : "";
  return `${count} fichier${count > 1 ? "s" : ""}${size}`;
}

function lastVerified(item: IsoItem): string {
  return item.sources.map(s => s.verified || "").sort().pop() ?? "";
}

async function toggleFavorite(item: IsoItem) {
  try { snapshot.value = await api<Snapshot>("favorite", { item: item.id }); }
  catch (error) { notifications.error("Favoris", errorText(error)); }
}

function iconError(event: Event) {
  (event.target as HTMLImageElement).style.display = "none";
}
</script>

<template>
  <div class="iso-stack">
    <div class="iso-chips">
      <button class="iso-chip" :class="{ 'iso-chip--on': family === 'all' }" @click="family = 'all'">Tout le catalogue ({{ items.length }})</button>
      <button class="iso-chip" :class="{ 'iso-chip--on': family === 'favorites' }" @click="family = 'favorites'">★ Favoris ({{ favorites.size }})</button>
      <button v-for="[key, label] in families" :key="key" class="iso-chip" :class="{ 'iso-chip--on': family === key }" @click="family = key">
        {{ label }} ({{ counts.get(key) }})
      </button>
    </div>

    <div class="iso-row">
      <div class="iso-row iso-field--grow" style="gap: 6px">
        <Search :size="14" style="color: var(--text-muted)" />
        <input v-model="query" class="iso-field iso-field--grow" placeholder="Rechercher un système, un outil, un fichier…" />
      </div>
      <select v-model="arch" class="iso-field" aria-label="Architecture">
        <option value="all">Toutes architectures</option>
        <option value="x64">x64</option><option value="x86">x86 (32 bits)</option>
        <option value="ARM64">ARM64</option><option value="ARM32">ARM 32 bits</option>
      </select>
      <select v-model="kind" class="iso-field" aria-label="Type de source">
        <option value="all">Tous types de source</option>
        <option v-for="(label, k) in KIND_LABELS" :key="k" :value="k">{{ label }}</option>
        <option value="unavailable">Sans fichier direct</option>
      </select>
      <select v-model="tag" class="iso-field" aria-label="Particularité">
        <option value="all">Toutes particularités</option>
        <option value="iso">Images ISO</option><option value="small">Images ≤ 1 Go</option>
        <option value="portable">Éditions portables</option><option value="legacy">Anciens PC (BIOS)</option>
      </select>
      <NButton variant="secondary" size="sm" @click="bundle?.show()"><PackagePlus :size="14" /> Trousse</NButton>
    </div>

    <div v-if="GROUPS[family]" class="iso-chips">
      <button class="iso-chip" :class="{ 'iso-chip--on': role === 'all' }" @click="role = 'all'">Tout</button>
      <button v-for="[value, label] in GROUPS[family]" :key="value" class="iso-chip" :class="{ 'iso-chip--on': role === value }" @click="role = value">{{ label }}</button>
    </div>

    <p class="iso-muted">{{ filtered.length }} fiche{{ filtered.length > 1 ? "s" : "" }}</p>

    <div class="iso-grid">
      <NCard v-for="item in filtered.slice(0, visible)" :key="item.id" hoverable>
        <div class="iso-card">
          <div class="iso-card-top">
            <div class="iso-icon">
              <img v-if="item.image" :src="`/icons/iso/${item.image}`" alt="" loading="lazy" @error="iconError" />
              <span v-else>{{ item.key.slice(0, 3).toUpperCase() }}</span>
            </div>
            <div>
              <div class="iso-tag">{{ familyLabel(item.family) }}</div>
              <h3>{{ item.name }}</h3>
            </div>
            <button class="iso-star" :class="{ 'iso-star--on': favorites.has(item.id) }"
              :aria-label="favorites.has(item.id) ? 'Retirer des favoris' : 'Ajouter aux favoris'" @click="toggleFavorite(item)">
              {{ favorites.has(item.id) ? "★" : "☆" }}
            </button>
          </div>
          <p class="iso-desc">{{ item.desc }}</p>
          <div class="iso-row">
            <NBadge v-for="k in kinds(item)" :key="k" :variant="KIND_VARIANTS[k]">{{ KIND_LABELS[k] }}</NBadge>
            <NBadge v-if="item.sources.some(s => s.sha256)" variant="success"><ShieldCheck :size="11" /> SHA-256</NBadge>
            <NBadge v-if="item.new" variant="accent">Nouveau</NBadge>
          </div>
          <div class="iso-foot">
            <span class="iso-muted">{{ footer(item) }}<template v-if="lastVerified(item)"> · vérifié le {{ lastVerified(item) }}</template></span>
            <NButton size="sm" :variant="item.sources.some(s => s.kind !== 'portal') ? 'primary' : 'secondary'" @click="detail = item">
              Voir les sources
            </NButton>
          </div>
        </div>
      </NCard>
    </div>
    <p v-if="!filtered.length" class="iso-small">Aucun résultat pour cette recherche.</p>
    <NButton v-if="filtered.length > visible" variant="ghost" @click="visible += 30">Afficher plus</NButton>

    <ItemDetail :item="detail" @close="detail = null" @inspect="f => { detail = null; emit('inspect', f); }"
      @bundle="(i, s) => { detail = null; bundle?.show(undefined, i, s); }" />
    <BundleDialog ref="bundle" @queued="emit('queued')" />
  </div>
</template>
