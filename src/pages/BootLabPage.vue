<script setup lang="ts">
// Port de TokaISO 7.9 (MIT, Cédrick Roy) : labo PXE + VM de test.
import { ref } from "vue";
import NTabs from "@/components/ui/NTabs.vue";
import NCard from "@/components/ui/NCard.vue";
import { Network, Server, FlaskConical, Activity, ClipboardList, Package, Info } from "lucide-vue-next";
import PxePanel from "@/components/boot-lab/PxePanel.vue";
import VmPanel from "@/components/boot-lab/VmPanel.vue";
import IsoTestPanel from "@/components/boot-lab/IsoTestPanel.vue";
import NetworkPanel from "@/components/boot-lab/NetworkPanel.vue";
import PlansPanel from "@/components/boot-lab/PlansPanel.vue";
import KitPanel from "@/components/boot-lab/KitPanel.vue";
import { installPrefill, type InstallSource } from "@/components/boot-lab/useBootLab";
import "@/components/boot-lab/boot-lab.css";

const tabs = [
  { id: "pxe", label: "Démarrage réseau PXE", icon: Network },
  { id: "iso", label: "Essai d’ISO", icon: FlaskConical },
  { id: "vm", label: "Machines virtuelles", icon: Server },
  { id: "network", label: "Réseau et disques", icon: Activity },
  { id: "plans", label: "Plans et labos", icon: ClipboardList },
  { id: "kit", label: "Trousse hors ligne", icon: Package },
];
const active = ref("pxe");
const networkPreset = ref<{ host: string; port: number } | null>(null);
const networkKey = ref(0);

function install(source: InstallSource) {
  installPrefill.value = { iso: source.iso ?? "", platform: source.platform, firmware: source.firmware };
  active.value = "vm";
}

function testPort(target: { host: string; port: number }) {
  networkPreset.value = target;
  networkKey.value++;
  active.value = "network";
}
</script>

<template>
  <div class="boot-lab-page">
    <div class="page-header">
      <h1>Labo PXE &amp; VM</h1>
      <p class="page-subtitle">Menus iPXE servis sur ton réseau local, VM de test VirtualBox / VMware, diagnostic réseau et trousses hors ligne</p>
    </div>

    <NCard>
      <div class="bl-banner">
        <Info :size="18" style="color: var(--accent-primary); flex-shrink: 0" />
        <p class="bl-text">
          Les VM créées ici portent le préfixe « NiTriTe-Test- » : seules celles-ci peuvent être retirées, tes autres VM ne sont
          jamais touchées. Port de TokaISO 7.9 (MIT, Cédrick Roy).
        </p>
      </div>
    </NCard>

    <NTabs v-model="active" :tabs="tabs" wrap>
      <PxePanel v-if="active === 'pxe'" @install="install" />
      <IsoTestPanel v-else-if="active === 'iso'" @install="install" />
      <VmPanel v-else-if="active === 'vm'" />
      <NetworkPanel v-else-if="active === 'network'" :key="networkKey" :preset="networkPreset" />
      <PlansPanel v-else-if="active === 'plans'" @test="testPort" />
      <KitPanel v-else />
    </NTabs>
  </div>
</template>

<style scoped>
.boot-lab-page { display: flex; flex-direction: column; gap: 16px; }
.page-header h1 { font-size: 24px; font-weight: 700; }
.page-subtitle { color: var(--text-muted); font-size: 13px; margin-top: 2px; }
</style>
