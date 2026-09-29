<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import { navigationSections } from "@/data/navigation";
import { Search } from "lucide-vue-next";

const emit = defineEmits<{ "open-search": [] }>();
const route = useRoute();

const currentPage = computed(() => {
  for (const section of navigationSections) {
    for (const item of section.items) {
      if (item.route === route.path) {
        return { label: item.label, section: section.title };
      }
    }
  }
  if (route.path === "/settings") {
    return { label: "Parametres", section: "" };
  }
  return { label: "Tableau de bord", section: "Systeme" };
});
</script>

<template>
  <header class="app-header">
    <div class="breadcrumb">
      <span v-if="currentPage.section" class="breadcrumb-section">{{ currentPage.section }}</span>
      <span v-if="currentPage.section" class="breadcrumb-sep">/</span>
      <span class="breadcrumb-page">{{ currentPage.label }}</span>
    </div>
    <div class="header-actions">
      <button class="search-btn" title="Recherche globale (Ctrl+K)" @click="emit('open-search')">
        <Search :size="16" />
        <span class="search-label">Rechercher...</span>
        <kbd class="search-kbd">Ctrl+K</kbd>
      </button>
    </div>
  </header>
</template>

<style scoped>
.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 24px;
  border-bottom: 1px solid var(--border);
  background: linear-gradient(180deg, var(--bg-secondary) 0%, color-mix(in srgb, var(--bg-primary) 95%, transparent) 100%);
  min-height: 52px;
  backdrop-filter: blur(20px) saturate(1.5);
  -webkit-backdrop-filter: blur(20px) saturate(1.5);
  position: relative;
  z-index: 50;
  /* Thin accent bottom line */
  box-shadow: 0 1px 0 var(--border), 0 4px 20px rgba(0,0,0,0.3);
}

.breadcrumb {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
}

.breadcrumb-section {
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 500;
  letter-spacing: 0.02em;
}

.breadcrumb-sep {
  color: var(--border-strong);
  font-size: 14px;
  line-height: 1;
  margin: 0 -2px;
}

.breadcrumb-page {
  color: var(--text-primary);
  font-weight: 600;
  font-size: 14px;
  letter-spacing: -0.1px;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.search-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  border: 1px solid var(--border-hover);
  border-radius: var(--radius-lg);
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  cursor: pointer;
  font-size: 12px;
  font-family: inherit;
  transition: all var(--transition-fast);
}

.search-btn:hover {
  border-color: rgba(249, 115, 22, 0.4);
  color: var(--text-primary);
  background: var(--bg-elevated);
  box-shadow: 0 0 0 3px var(--accent-muted), var(--shadow-sm);
}

.search-label {
  min-width: 100px;
  text-align: left;
}

.search-kbd {
  font-size: 10px;
  padding: 2px 6px;
  border: 1px solid var(--border-hover);
  border-radius: 5px;
  background: var(--bg-secondary);
  color: var(--text-secondary);
  font-family: inherit;
  letter-spacing: 0.02em;
}
</style>
