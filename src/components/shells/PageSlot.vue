<script setup lang="ts">
/**
 * Emplacement de la page dans une coque. App.vue y téléporte la zone de page
 * (router-view + keep-alive) : changer de modèle DÉPLACE la page au lieu de la
 * recréer, donc une opération en cours (scan, installation…) et l'état des
 * pages ne sont pas perdus.
 */
import { useUiModelStore } from "@/stores/uiModel";
const store = useUiModelStore();
</script>

<template>
  <div class="sh-page" :data-page-slot="store.model"><slot /></div>
</template>

<style scoped>
.sh-page { flex: 1; min-height: 0; min-width: 0; display: flex; flex-direction: column; position: relative; }
</style>
