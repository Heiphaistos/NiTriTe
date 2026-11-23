<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : diagnostic en lecture seule de la clé (usb_health.js).
import { ref, watch } from "vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import { Activity, BookOpen } from "lucide-vue-next";
import { useUsbWorkshop, displayBytes, errorText } from "./useUsbWorkshop";

const ws = useUsbWorkshop();
const { device, busy } = ws;

interface SmartResult { smart: string; source: string; windowsHealth: string; counters: Record<string, number>; alerts: boolean }
interface ReadResult { read: string; bytes: number }

const pending = ref("");
const smartResult = ref<SmartResult | null>(null);
const readResult = ref<ReadResult | null>(null);

const smartLabels: Record<string, string> = { passed: "Aucune défaillance signalée par SMART", failed: "SMART signale une défaillance",
  unavailable: "Données SMART indisponibles" };
const healthLabels: Record<string, string> = { normal: "Normal", warning: "Avertissement", failed: "Défaillance", unavailable: "Indisponible" };
const readLabels: Record<string, string> = { passed: "Échantillon lu", failed: "Erreur pendant la lecture", unavailable: "Lecture non vérifiée" };
const counterLabels: Record<string, string> = { temperature: "Température", wear: "Usure estimée", hours: "Temps de fonctionnement",
  readErrors: "Erreurs de lecture non corrigées", writeErrors: "Erreurs d’écriture non corrigées" };
const counterUnits: Record<string, string> = { temperature: " °C", wear: " %", hours: " h" };

watch(device, () => { smartResult.value = null; readResult.value = null; });

async function perform(kind: "smart" | "read") {
  const target = device.value;
  if (pending.value || !target) return;
  pending.value = kind;
  try {
    const result = await ws.run<SmartResult & ReadResult>("usb_ws_health", { id: target, kind }, kind === "smart" ? 95000 : 155000);
    if (device.value !== target) return;
    if (kind === "smart") smartResult.value = result;
    else readResult.value = result;
  } catch (error) {
    ws.notify(errorText(error));
  } finally {
    pending.value = "";
  }
}
</script>

<template>
  <section>
    <h3>Santé et SMART du support USB</h3>
    <p class="usbws-muted">Le diagnostic SMART dépend des données transmises par la clé ou son adaptateur USB.</p>
    <div class="usbws-actions">
      <NButton variant="secondary" size="sm" :loading="pending === 'smart'" :disabled="!device || !!pending || busy" @click="perform('smart')">
        <Activity :size="14" /> Lire les données SMART
      </NButton>
      <NButton variant="secondary" size="sm" :loading="pending === 'read'" :disabled="!device || !!pending || busy" @click="perform('read')">
        <BookOpen :size="14" /> Tester la lecture sans écrire
      </NButton>
    </div>
    <p class="usbws-muted">La lecture teste les premiers 8 Mio au maximum. Windows peut demander une autorisation.</p>

    <div aria-live="polite">
      <p v-if="pending" class="usbws-muted">{{ pending === "smart" ? "Lecture SMART en cours…" : "Lecture de l’échantillon en cours…" }}</p>
      <template v-if="smartResult">
        <div class="usbws-row">
          <div class="usbws-row-main"><strong>{{ smartLabels[smartResult.smart] }}</strong><span>{{ smartResult.source }}</span></div>
          <NBadge :variant="smartResult.smart === 'failed' ? 'danger' : smartResult.smart === 'passed' ? 'success' : 'neutral'">SMART</NBadge>
        </div>
        <div class="usbws-row">
          <div class="usbws-row-main"><strong>État signalé par Windows</strong><span>{{ healthLabels[smartResult.windowsHealth] }}</span></div>
        </div>
        <p v-if="smartResult.alerts" class="usbws-warning">Les indicateurs signalent un problème à examiner.</p>
        <template v-for="(value, key) in smartResult.counters" :key="key">
          <div v-if="counterLabels[key]" class="usbws-row">
            <div class="usbws-row-main"><strong>{{ counterLabels[key] }}</strong><span>{{ value.toLocaleString("fr-FR") + (counterUnits[key] || "") }}</span></div>
          </div>
        </template>
      </template>
      <div v-if="readResult" class="usbws-row">
        <div class="usbws-row-main">
          <strong>{{ readLabels[readResult.read] }}</strong>
          <span v-if="readResult.bytes">Taille ciblée · {{ displayBytes(readResult.bytes) }}</span>
        </div>
      </div>
      <p v-if="!device" class="usbws-muted">Choisis la clé USB à examiner.</p>
    </div>
  </section>
</template>
