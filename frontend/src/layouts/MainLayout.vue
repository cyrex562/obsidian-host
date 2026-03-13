<template>
  <v-navigation-drawer
    v-model="sidebarOpen"
    :width="sidebarWidth"
    permanent
    rail-width="0"
    style="background: rgb(var(--v-theme-surface)); border-right: 1px solid rgb(var(--v-theme-border));"
  >
    <!-- Vault selector row -->
    <div class="d-flex align-center pa-2 gap-2" style="border-bottom: 1px solid rgb(var(--v-theme-border));">
      <v-select
        :items="vaultsStore.vaults"
        :item-title="(v) => v.path_exists === false ? v.name + ' (missing)' : v.name"
        item-value="id"
        :model-value="vaultsStore.activeVaultId"
        placeholder="Select vault…"
        hide-details
        density="compact"
        variant="outlined"
        style="flex: 1; min-width: 0;"
        @update:model-value="onVaultChange"
      />
      <v-btn icon="mdi-cog" size="small" @click="vaultManagerOpen = true" />
    </div>

    <!-- Sidebar action buttons -->
    <SidebarActions />

    <!-- File tree -->
    <div style="flex: 1; overflow-y: auto; overflow-x: hidden;">
      <FileTree v-if="vaultsStore.activeVaultId" />
      <div v-else class="pa-4 text-secondary text-caption text-center">
        Select a vault to start.
      </div>
    </div>
  </v-navigation-drawer>

  <!-- Top app bar -->
  <TopBar @open-search="searchOpen = true" @open-plugins="pluginsOpen = true" />

  <!-- Main content: tab bars + editor panes -->
  <v-main style="height: 100vh; display: flex; flex-direction: column; overflow: hidden;">
    <PaneContainer />
  </v-main>

  <!-- Resize handle for sidebar -->
  <div
    class="sidebar-resize-handle"
    @mousedown="startResize"
  />

  <!-- Modals -->
  <VaultManager v-model="vaultManagerOpen" />
  <SearchModal v-model="searchOpen" />
  <QuickSwitcher v-model="quickSwitcherOpen" />
  <PluginManager v-model="pluginsOpen" />
  <TemplateSelector v-model="uiStore.templateSelectorOpen" />
  <ConflictResolver v-model="uiStore.conflictResolverOpen" />
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { useRouter } from 'vue-router';
import { ApiError } from '@/api/client';
import { useAuthStore } from '@/stores/auth';
import { useVaultsStore } from '@/stores/vaults';
import { useFilesStore } from '@/stores/files';
import { useTabsStore } from '@/stores/tabs';
import { useUiStore } from '@/stores/ui';
import { useWebSocket } from '@/composables/useWebSocket';

import TopBar from '@/components/TopBar.vue';
import SidebarActions from '@/components/sidebar/SidebarActions.vue';
import FileTree from '@/components/sidebar/FileTree.vue';
import PaneContainer from '@/components/tabs/PaneContainer.vue';
import VaultManager from '@/components/modals/VaultManager.vue';
import SearchModal from '@/components/modals/SearchModal.vue';
import QuickSwitcher from '@/components/modals/QuickSwitcher.vue';
import PluginManager from '@/components/modals/PluginManager.vue';
import TemplateSelector from '@/components/modals/TemplateSelector.vue';
import ConflictResolver from '@/components/modals/ConflictResolver.vue';

const vaultsStore = useVaultsStore();
const filesStore = useFilesStore();
const tabsStore = useTabsStore();
const uiStore = useUiStore();
const authStore = useAuthStore();
const router = useRouter();

const sidebarOpen = ref(true);
const sidebarWidth = ref(280);
const vaultManagerOpen = ref(false);
const searchOpen = ref(false);
const quickSwitcherOpen = ref(false);
const pluginsOpen = ref(false);

onMounted(async () => {
  try {
    await authStore.ensureFresh();
    await authStore.loadProfile();
  } catch {
    await authStore.logout();
    await router.replace({
      path: '/login',
      query: { redirect: router.currentRoute.value.fullPath || '/' },
    });
    return;
  }

  useWebSocket();

  await vaultsStore.loadVaults();
  if (vaultsStore.activeVaultId) {
    await filesStore.loadTree(vaultsStore.activeVaultId);
    await filesStore.loadRecentFiles(vaultsStore.activeVaultId);
  }

  // Keyboard shortcut: Ctrl+P → quick switcher
  window.addEventListener('keydown', onGlobalKeydown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', onGlobalKeydown);
});

async function onVaultChange(id: string) {
  vaultsStore.setActiveVault(id);
  if (id) {
    await filesStore.loadTree(id);
    await filesStore.loadRecentFiles(id);
  }
}

function onGlobalKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
    e.preventDefault();
    void saveActiveTabNow();
    return;
  }

  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'p') {
    e.preventDefault();
    quickSwitcherOpen.value = true;
  }
}

async function saveActiveTabNow() {
  const vaultId = vaultsStore.activeVaultId;
  const tab = tabsStore.activeTab;
  if (!vaultId || !tab || !tab.filePath || !tab.isDirty) return;

  try {
    const saved = await filesStore.writeFile(vaultId, tab.filePath, {
      content: tab.content,
      last_modified: tab.modified || undefined,
      frontmatter: tab.frontmatter,
    });
    tabsStore.markTabClean(tab.id, saved.modified);
  } catch (error) {
    if (error instanceof ApiError && error.status === 409) {
      const latest = await filesStore.readFile(vaultId, tab.filePath);
      uiStore.openConflictResolver({
        tabId: tab.id,
        filePath: tab.filePath,
        yourVersion: tab.content,
        serverVersion: latest.content,
        serverModified: latest.modified,
      });
      return;
    }
    throw error;
  }
}

// ── Sidebar resize ────────────────────────────────────────────────────────────

let resizing = false;
let resizeStartX = 0;
let resizeStartWidth = 280;

function startResize(e: MouseEvent) {
  resizing = true;
  resizeStartX = e.clientX;
  resizeStartWidth = sidebarWidth.value;
  window.addEventListener('mousemove', onResize);
  window.addEventListener('mouseup', stopResize);
}

function onResize(e: MouseEvent) {
  if (!resizing) return;
  const delta = e.clientX - resizeStartX;
  sidebarWidth.value = Math.max(160, Math.min(600, resizeStartWidth + delta));
}

function stopResize() {
  resizing = false;
  window.removeEventListener('mousemove', onResize);
  window.removeEventListener('mouseup', stopResize);
}
</script>

<style scoped>
.sidebar-resize-handle {
  position: fixed;
  left: v-bind(sidebarWidth + 'px');
  top: 0;
  width: 4px;
  height: 100vh;
  cursor: col-resize;
  z-index: 200;
  transition: background 0.15s;
}
.sidebar-resize-handle:hover {
  background: rgb(var(--v-theme-primary));
}
</style>
