<script setup lang="ts">
// Activation — Réactivation Windows (maintenance technicien).
// Une seule voie retenue : HWID permanent (via MAS), en un clic.
// Affiche l'état RÉEL renvoyé par slmgr/CIM ; aucune donnée inventée.
import { ref, onMounted, computed } from "vue";
import { invoke } from "@/utils/invoke";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import NSpinner from "@/components/ui/NSpinner.vue";
import { useNotificationStore } from "@/stores/notifications";
import { KeyRound, ShieldCheck, RefreshCw, Fingerprint } from "lucide-vue-next";

const notify = useNotificationStore();

interface DetectedEdition { edition_id: string; product_name: string; gvlk: string | null; gvlk_label: string | null; }
interface LicenseStatus {
  name: string; description: string; license_status: number; license_status_text: string;
  grace_minutes: number; grace_days: number; channel: string; partial_key: string; activated: boolean;
}
interface ActivationStatus { edition: DetectedEdition; license: LicenseStatus; dlv_raw: string; xpr_raw: string; }
interface Step { label: string; success: boolean; output: string; }

const status = ref<ActivationStatus | null>(null);
const loading = ref(false);
const working = ref(false);

const activated = computed(() => status.value?.license.activated ?? false);
const badgeVariant = computed(() => (activated.value ? "success" : "warning"));

async function refresh() {
  loading.value = true;
  try {
    status.value = await invoke<ActivationStatus>("activation_status", undefined, 60_000);
  } catch (e) {
    notify.error("État indisponible", String(e));
  } finally {
    loading.value = false;
  }
}

async function activateHwid() {
  if (!confirm(
    "Lancer l'activation permanente HWID via MAS (un clic) ?\n\n" +
    "Une élévation administrateur va être demandée, puis MAS s'exécute en mode HWID " +
    "sans aucune autre question et se referme seul. L'activation HWID est permanente " +
    "et liée au matériel de ce PC."
  )) return;
  working.value = true;
  try {
    const s = await invoke<Step>("activation_hwid", undefined, 30_000);
    if (s.success) notify.success("HWID", s.output);
    else notify.error("HWID", s.output);
    await refresh();
  } catch (e) {
    notify.error("Échec HWID", String(e));
  } finally {
    working.value = false;
  }
}

onMounted(refresh);
</script>

<template>
  <div class="activation-page">
    <header class="page-head">
      <div>
        <h1><KeyRound :size="22" /> Activation Windows</h1>
        <p class="sub">Réactivation permanente (HWID) après réparation ou réinstallation, en un clic.</p>
      </div>
      <NButton variant="ghost" :loading="loading" @click="refresh">
        <RefreshCw :size="16" /> Rafraîchir l'état
      </NButton>
    </header>

    <!-- État réel -->
    <NCard>
      <template #header><strong>État actuel</strong></template>
      <div v-if="loading" class="center"><NSpinner /> Lecture de l'état…</div>
      <div v-else-if="status" class="state-grid">
        <div><span class="k">Édition</span><span class="v">{{ status.edition.product_name || status.edition.edition_id || "Inconnue" }}</span></div>
        <div><span class="k">Statut</span><span class="v"><NBadge :variant="badgeVariant">{{ status.license.license_status_text }}</NBadge></span></div>
        <div><span class="k">Canal</span><span class="v">{{ status.license.channel || "—" }}</span></div>
        <div><span class="k">Clé posée</span><span class="v">{{ status.license.partial_key ? "…" + status.license.partial_key : "aucune" }}</span></div>
        <div v-if="!activated && status.license.grace_days > 0">
          <span class="k">Jours restants</span><span class="v">{{ status.license.grace_days }} j</span>
        </div>
        <div><span class="k">GVLK détectée</span><span class="v">{{ status.edition.gvlk_label || "non prise en charge" }}</span></div>
      </div>
      <pre v-if="status?.xpr_raw" class="raw">{{ status.xpr_raw }}</pre>
    </NCard>

    <!-- Action -->
    <div class="actions">
      <NCard>
        <template #header><strong><Fingerprint :size="16" /> Activation permanente HWID (via MAS)</strong></template>
        <p>Active définitivement Windows via MAS en mode HWID (lié au matériel). Un clic : élévation administrateur, puis exécution silencieuse sans aucune autre question ; la fenêtre se referme seule à la fin.</p>
        <NButton variant="success" :loading="working" @click="activateHwid">
          <ShieldCheck :size="16" /> Activer (un clic)
        </NButton>
      </NCard>
    </div>

    <!-- Diagnostic détaillé -->
    <NCard v-if="status?.dlv_raw">
      <template #header><strong>Diagnostic détaillé (slmgr /dlv)</strong></template>
      <pre class="raw">{{ status.dlv_raw }}</pre>
    </NCard>
  </div>
</template>

<style scoped>
.activation-page { display: flex; flex-direction: column; gap: 16px; padding: 4px; }
.page-head { display: flex; justify-content: space-between; align-items: flex-start; gap: 16px; flex-wrap: wrap; }
.page-head h1 { display: flex; align-items: center; gap: 8px; font-size: 1.4rem; margin: 0; }
.sub { color: var(--text-secondary); margin: 4px 0 0; }
.center { display: flex; align-items: center; gap: 8px; color: var(--text-secondary); padding: 8px 0; }
.state-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 12px; }
.state-grid .k { display: block; color: var(--text-secondary); font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.04em; }
.state-grid .v { display: block; font-weight: 600; margin-top: 2px; }
.actions { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 16px; }
.actions p { color: var(--text-secondary); font-size: 0.9rem; margin: 0 0 12px; }
.actions code { background: var(--bg-tertiary); padding: 1px 5px; border-radius: 4px; }
.raw { background: var(--bg-tertiary); border: 1px solid var(--border); border-radius: 8px; padding: 10px; font-size: 0.8rem; white-space: pre-wrap; overflow-x: auto; margin: 8px 0 0; }
header strong { display: inline-flex; align-items: center; gap: 6px; }
</style>
