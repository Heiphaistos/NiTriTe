<script setup lang="ts">
/** Modèle « Classique » : l'interface historique de NiTriTe, à l'identique. */
import { computed, inject, type Ref, ref } from "vue";
import AppSidebar from "@/components/layout/AppSidebar.vue";
import AppHeader from "@/components/layout/AppHeader.vue";
import AppStatusBar from "@/components/layout/AppStatusBar.vue";
import { useLayoutStore } from "@/stores/layoutStore";

const layoutStore = useLayoutStore();
const sidebarCollapsed = inject<Ref<boolean>>("sidebarCollapsed", ref(false));
const toggleSidebar = inject<() => void>("toggleSidebar", () => {});
const openSearch = inject<() => void>("openSearch", () => {});

const isRightSidebar      = computed(() => layoutStore.state.sidebarPosition === "right");
const currentSidebarWidth = computed(() => sidebarCollapsed.value ? 64 : layoutStore.sidebarWidthPx);
</script>

<template>
  <div
    class="app-layout"
    :class="[`sidebar-pos-${layoutStore.state.sidebarPosition}`, `density-${layoutStore.state.density}`]"
    :data-density="layoutStore.state.density"
    :data-sidebar-pos="layoutStore.state.sidebarPosition"
  >
    <AppSidebar
      :collapsed="sidebarCollapsed"
      :position="layoutStore.state.sidebarPosition"
      :width="layoutStore.state.sidebarWidth"
      :mode="layoutStore.state.sidebarMode"
      @toggle="toggleSidebar"
    />
    <div
      class="app-main"
      :class="{ 'sidebar-collapsed': sidebarCollapsed }"
      :style="{
        [isRightSidebar ? 'marginRight' : 'marginLeft']: `${currentSidebarWidth}px`,
        [isRightSidebar ? 'marginLeft'  : 'marginRight']: '0',
      }"
    >
      <AppHeader v-if="layoutStore.state.headerVisible" @open-search="openSearch" />
      <slot />
      <AppStatusBar />
    </div>
  </div>
</template>

<style scoped>
.app-layout { display:flex; height:100%; overflow:hidden; }
.sidebar-pos-right { flex-direction:row-reverse; }
.app-main { flex:1; display:flex; flex-direction:column; transition:margin var(--transition-normal); min-width:0; }
</style>
