<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : carte d'une VM d'essai (showIso + vm75ShowTest).
import { ref } from "vue";
import NButton from "@/components/ui/NButton.vue";
import NBadge from "@/components/ui/NBadge.vue";
import { useNotificationStore } from "@/stores/notifications";
import { call, launchMessage, platformNames, vmStates, type ControlResult, type InstallSource, type Machine } from "./useBootLab";

const props = defineProps<{ machine: Machine }>();
const emit = defineEmits<{ update: [machine: Machine]; removed: [id: string]; install: [source: InstallSource] }>();
const notify = useNotificationStore();
const pending = ref("");

async function act(action: string, timeout = 45000) {
  if (pending.value) return;
  pending.value = action;
  const result = await call<ControlResult>("boot_lab_vm_action", { action, id: props.machine.id }, timeout);
  pending.value = "";
  if (!result) return;
  if (result.removed) {
    notify.success("VM d’essai retirée.");
    emit("removed", props.machine.id);
    return;
  }
  if (result.machine) emit("update", result.machine);
  const messages: Record<string, string> = {
    "vm-start": "VM allumée",
    "vm-console": "Consulte le menu de démarrage dans la console de la VM.",
    "vm-stop-test": "VM d’essai arrêtée.",
    "vm-test-confirm": "Menu de démarrage confirmé dans l’historique.",
    "vm-status": vmStates[result.machine?.state ?? ""] ?? "",
  };
  notify.info(launchMessage(result) || messages[action] || "Opération terminée.");
}
</script>

<template>
  <div class="bl-row">
    <div class="bl-row-main">
      <span class="bl-row-name">{{ machine.file }}</span>
      <span class="bl-muted">
        {{ [platformNames[machine.platform], ...(machine.boot || [])].join(" · ") }} · {{ machine.name }}
      </span>
      <span class="bl-muted">
        <NBadge :variant="machine.state === 'imported' ? 'neutral' : machine.state === 'unknown' ? 'warning' : 'success'">
          {{ vmStates[machine.state] || machine.state }}
        </NBadge>
        <NBadge v-if="machine.menu_at" variant="success">Menu confirmé</NBadge>
        VM d’essai prête. Le menu de démarrage se vérifie dans la console.
      </span>
    </div>
    <div class="bl-actions">
      <NButton size="sm" :loading="pending === 'vm-console'" @click="act('vm-console')">Ouvrir la console</NButton>
      <NButton v-if="machine.platform === 'vmware'" size="sm" variant="secondary" :loading="pending === 'vm-start'" @click="act('vm-start', 90000)">
        Démarrer sans fenêtre
      </NButton>
      <NButton size="sm" variant="secondary" :loading="pending === 'vm-status'" @click="act('vm-status')">Vérifier l’état</NButton>
      <NButton size="sm" variant="secondary" :loading="pending === 'vm-stop-test'" @click="act('vm-stop-test')">Arrêter le test</NButton>
      <NButton v-if="!machine.menu_at" size="sm" variant="secondary" :loading="pending === 'vm-test-confirm'" @click="act('vm-test-confirm')">
        Confirmer le menu de démarrage
      </NButton>
      <NButton size="sm" variant="secondary" @click="emit('install', machine)">Créer une VM avec cette ISO</NButton>
      <NButton size="sm" variant="danger" :loading="pending === 'vm-discard-test'" @click="act('vm-discard-test')">Retirer le test</NButton>
    </div>
  </div>
</template>
