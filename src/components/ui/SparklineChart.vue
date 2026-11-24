<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  data: number[];
  color?: string;
  height?: number;
  fill?: boolean;
  label?: string;
  maxPoints?: number; // limite dynamique (défaut 60)
}>();

const h = computed(() => props.height ?? 40);
const w = 120;
const color = computed(() => props.color ?? "var(--accent-primary)");

// Fenêtre glissante : on affiche les N derniers points
const visibleData = computed(() => {
  const max = props.maxPoints ?? 60;
  const d = props.data;
  return d.length > max ? d.slice(d.length - max) : d;
});

const points = computed(() => {
  const d = visibleData.value;
  if (!d || d.length < 2) return "";
  const min = Math.min(...d);
  const max = Math.max(...d);
  const range = max - min || 1;
  const step = w / (d.length - 1);
  return d
    .map((v, i) => {
      const x = i * step;
      const y = h.value - ((v - min) / range) * (h.value - 4) - 2;
      return `${x},${y}`;
    })
    .join(" ");
});

const fillPath = computed(() => {
  if (!points.value) return "";
  const d = visibleData.value;
  const step = w / (d.length - 1);
  const last = `${(d.length - 1) * step},${h.value}`;
  // Un seul « L » par point : « L L x,y » est un chemin SVG invalide (zone de remplissage absente).
  const line = points.value.split(" ").map((p) => `L ${p}`).join(" ");
  return `M 0,${h.value} ${line} L ${last} Z`;
});

// Coordonnées du dernier point (dot indicateur)
const lastDot = computed(() => {
  const d = visibleData.value;
  if (!d || d.length < 2) return null;
  const min = Math.min(...d);
  const max = Math.max(...d);
  const range = max - min || 1;
  const step = w / (d.length - 1);
  const i = d.length - 1;
  const x = i * step;
  const y = h.value - ((d[i] - min) / range) * (h.value - 4) - 2;
  return { x, y };
});
</script>

<template>
  <div class="spark-wrap">
    <svg :width="w" :height="h" :viewBox="`0 0 ${w} ${h}`" preserveAspectRatio="none">
      <defs>
        <linearGradient :id="`grad-${label}`" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" :stop-color="color" stop-opacity="0.35" />
          <stop offset="100%" :stop-color="color" stop-opacity="0.02" />
        </linearGradient>
      </defs>
      <path
        v-if="fill && fillPath"
        :d="fillPath"
        :fill="`url(#grad-${label})`"
        stroke="none"
      />
      <polyline
        v-if="points"
        :points="points"
        fill="none"
        :stroke="color"
        stroke-width="1.5"
        stroke-linejoin="round"
        stroke-linecap="round"
      />
      <!-- Dot indicateur sur le dernier point -->
      <circle
        v-if="lastDot"
        :cx="lastDot.x"
        :cy="lastDot.y"
        r="2.5"
        :fill="color"
        opacity="0.9"
      />
    </svg>
  </div>
</template>

<style scoped>
.spark-wrap { display: flex; align-items: center; }
svg { overflow: visible; }
</style>
