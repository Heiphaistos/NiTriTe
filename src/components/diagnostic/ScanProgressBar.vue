<script setup lang="ts">
import NSpinner from "@/components/ui/NSpinner.vue";
import NProgress from "@/components/ui/NProgress.vue";
defineProps<{ step: string; progress: number }>();
</script>

<template>
  <div class="scan-progress" style="display:flex;flex-direction:column;gap:10px">
    <div style="display:flex;align-items:center;gap:8px;font-size:13px;color:var(--text-secondary)">
      <NSpinner :size="16" /><span>{{ step }}</span>
    </div>
    <NProgress :value="progress" showLabel size="lg" color="accent" />
    <p class="muted" style="font-size:12px">Analyse complète en cours — cela peut prendre 1 à 3 minutes...</p>
  </div>
</template>
