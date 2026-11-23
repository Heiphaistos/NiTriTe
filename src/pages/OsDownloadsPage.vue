<script setup lang="ts">
// Téléchargement OS : sélection NiTriTe (contenu d'origine, onglet par défaut) +
// catalogue et téléchargeur portés de TokaISO 7.9 (MIT, Cédrick Roy).
import { computed, nextTick, onMounted, ref } from "vue";
import NTabs from "@/components/ui/NTabs.vue";
import { Star, LayoutGrid, Download, Library, Wrench } from "lucide-vue-next";
import SelectionTab from "@/components/os-downloads/SelectionTab.vue";
import CatalogTab from "@/components/os-downloads/CatalogTab.vue";
import DownloadsTab from "@/components/os-downloads/DownloadsTab.vue";
import LibraryTab from "@/components/os-downloads/LibraryTab.vue";
import ToolsVerify from "@/components/os-downloads/ToolsVerify.vue";
import ToolsHealth from "@/components/os-downloads/ToolsHealth.vue";
import ToolsAdvisor from "@/components/os-downloads/ToolsAdvisor.vue";
import ToolsPersonal from "@/components/os-downloads/ToolsPersonal.vue";
import { useIsoCatalog, type MediaFile } from "@/components/os-downloads/useIsoCatalog";
import "@/components/os-downloads/iso.css";

const { activeCount, loading, loadError, loadCatalogue } = useIsoCatalog();
const tab = ref("selection");
const tools = ref("verify");
const library = ref<InstanceType<typeof LibraryTab> | null>(null);

const tabs = computed(() => [
  { id: "selection", label: "Sélection NiTriTe", icon: Star },
  { id: "catalog", label: "Catalogue complet", icon: LayoutGrid },
  { id: "downloads", label: activeCount.value ? `Téléchargements (${activeCount.value})` : "Téléchargements", icon: Download },
  { id: "library", label: "Bibliothèque", icon: Library },
  { id: "tools", label: "Outils", icon: Wrench },
]);
const toolTabs = [
  { id: "verify", label: "Vérification" }, { id: "health", label: "Santé des sources" },
  { id: "advisor", label: "Conseiller et suivis" }, { id: "personal", label: "Mon catalogue" },
];

onMounted(loadCatalogue);

async function inspect(file: MediaFile) {
  tab.value = "library";
  await nextTick();
  library.value?.inspect(file);
}
</script>

<template>
  <div class="os-downloads-page">
    <div class="page-header">
      <h1>Téléchargement OS</h1>
      <p class="page-subtitle">ISO officielles et catalogue complet — Windows, macOS, Linux, utilitaires</p>
    </div>

    <NTabs v-model="tab" :tabs="tabs" wrap>
      <SelectionTab v-if="tab === 'selection'" />
      <template v-else>
        <p v-if="loading" class="iso-muted">Chargement du catalogue…</p>
        <p v-else-if="loadError" class="iso-error">{{ loadError }}</p>
        <CatalogTab v-if="tab === 'catalog'" @inspect="inspect" @queued="tab = 'downloads'" />
        <DownloadsTab v-else-if="tab === 'downloads'" />
        <LibraryTab v-else-if="tab === 'library'" ref="library" />
        <NTabs v-else-if="tab === 'tools'" v-model="tools" :tabs="toolTabs">
          <ToolsVerify v-if="tools === 'verify'" />
          <ToolsHealth v-else-if="tools === 'health'" />
          <ToolsAdvisor v-else-if="tools === 'advisor'" />
          <ToolsPersonal v-else />
        </NTabs>
      </template>
    </NTabs>
  </div>
</template>

<style scoped>
.os-downloads-page { display: flex; flex-direction: column; gap: 16px; }
.page-header h1 { font-size: 24px; font-weight: 700; }
.page-subtitle { color: var(--text-muted); font-size: 13px; margin-top: 2px; }
</style>
