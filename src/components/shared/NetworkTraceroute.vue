<script setup lang="ts">
import { ref } from "vue";
import { invoke, isTauriContext } from "@/utils/invoke";
import { useNotificationStore } from "@/stores/notifications";
import NCard from "@/components/ui/NCard.vue";
import NButton from "@/components/ui/NButton.vue";
import NSpinner from "@/components/ui/NSpinner.vue";
import NBadge from "@/components/ui/NBadge.vue";
import NInput from "@/components/ui/NInput.vue";
import { Route } from "lucide-vue-next";

interface TracertHop { hop: number; address: string; ms: number; }

const tracerouteHost = ref("8.8.8.8");
const tracerouting   = ref(false);
const hops           = ref<TracertHop[]>([]);
const ran            = ref(false);

async function runTraceroute() {
  if (!tracerouteHost.value.trim()) return;
  tracerouting.value = true;
  hops.value = [];
  ran.value = false;
  try {
    hops.value = await invoke<TracertHop[]>("run_traceroute", {
      host: tracerouteHost.value.trim(),
    }, 130_000); // 20 sauts x 3 essais x 1 s dépassent les 30 s par défaut ; borné à 120 s côté Rust
    ran.value = true;
  } catch (e: unknown) {
    if (isTauriContext()) {
      const msg = e instanceof Error ? e.message : String(e);
      useNotificationStore().error("Traceroute", msg.slice(0, 120));
    } else {
      ran.value = true;
    }
  }
  tracerouting.value = false;
}
</script>

<template>
  <NCard>
    <template #header>
      <div class="section-header">
        <Route :size="16" />
        <span>Traceroute</span>
        <NBadge v-if="hops.length" variant="neutral" style="margin-left:auto">
          {{ hops.length }} saut(s)
        </NBadge>
      </div>
    </template>
    <div class="traceroute-zone">
      <div class="input-row">
        <NInput
          v-model="tracerouteHost"
          placeholder="IP ou domaine cible"
          @keyup.enter="runTraceroute"
        />
        <NButton variant="primary" size="md" :loading="tracerouting" @click="runTraceroute">
          <Route :size="14" />
          Tracer
        </NButton>
      </div>
      <p class="note">Limite a 20 sauts — timeout 1s/hop. Peut prendre 20-30 secondes.</p>

      <div v-if="tracerouting" class="loading-row">
        <NSpinner :size="16" />
        <span>Traceroute en cours... (20-30s)</span>
      </div>

      <div v-if="hops.length" class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th style="width:50px">Hop</th>
              <th>Adresse</th>
              <th style="width:100px">Latence</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="hop in hops" :key="hop.hop">
              <td class="font-mono muted">{{ hop.hop }}</td>
              <td class="font-mono">{{ hop.address === "*" ? "* (pas de reponse)" : hop.address }}</td>
              <td>
                <span
                  v-if="hop.address !== '*' && hop.ms > 0"
                  class="hop-lat"
                  :style="{
                    color: hop.ms < 20 ? 'var(--success)' : hop.ms < 80 ? 'var(--warning)' : 'var(--danger)'
                  }"
                >{{ hop.ms }} ms</span>
                <span v-else class="font-mono muted">—</span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div v-else-if="ran && !tracerouting" class="empty-msg">
        Aucun saut retourne. La cible est peut-etre inaccessible.
      </div>
    </div>
  </NCard>
</template>

<style scoped>
.section-header { display:flex; align-items:center; gap:8px; }
.traceroute-zone { display:flex; flex-direction:column; gap:10px; }
.input-row { display:flex; gap:8px; align-items:flex-end; }
.input-row > :first-child { flex:1; }
.note { font-size:11px; color:var(--text-muted); margin:0; line-height:1.5; }
.loading-row { display:flex; align-items:center; gap:8px; font-size:13px; color:var(--text-muted); }
.empty-msg { font-size:13px; color:var(--text-muted); padding:8px 0; }
.table-wrap { overflow-x:auto; }
.data-table { width:100%; border-collapse:collapse; font-size:13px; }
.data-table th { text-align:left; padding:8px 12px; color:var(--text-muted); font-size:12px; border-bottom:1px solid var(--border); }
.data-table td { padding:8px 12px; color:var(--text-secondary); border-bottom:1px solid var(--border); }
.data-table tbody tr:hover { background:var(--bg-tertiary); }
.font-mono { font-family:"JetBrains Mono",monospace; font-size:12px; }
.muted { color:var(--text-muted) !important; }
.hop-lat { font-family:"JetBrains Mono",monospace; font-size:12px; font-weight:600; }
</style>
