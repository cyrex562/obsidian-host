<template>
  <div class="sidebar-actions d-flex align-center pa-1 gap-1" style="border-bottom: 1px solid rgb(var(--v-theme-border));">
    <v-btn
      icon="mdi-graph-outline"
      size="small"
      density="compact"
      title="Graph view"
      @click="openGraph"
    />
    <v-btn
      icon="mdi-cube-plus-outline"
      size="small"
      density="compact"
      title="New entity"
      @click="newEntityDialog = true"
    />
    <v-btn
      icon="mdi-file-plus-outline"
      size="small"
      density="compact"
      title="New note"
      @click="newNote"
    />
    <v-btn
      icon="mdi-folder-plus-outline"
      size="small"
      density="compact"
      title="New folder"
      @click="newFolder"
    />
    <v-btn
      icon="mdi-refresh"
      size="small"
      density="compact"
      title="Refresh file tree"
      :loading="filesStore.loading"
      @click="refresh"
    />
    <v-btn
      icon="mdi-file-document-plus-outline"
      size="small"
      density="compact"
      title="Insert template"
      @click="uiStore.openTemplateSelector()"
    />
    <v-btn
      icon="mdi-folder-upload-outline"
      size="small"
      density="compact"
      title="Import files or folders"
      @click="uiStore.openImportDialog()"
    />
    <v-menu>
      <template #activator="{ props: menuProps }">
        <v-btn
          icon="mdi-export"
          size="small"
          density="compact"
          title="Export vault or folder"
          v-bind="menuProps"
        />
      </template>
      <v-list density="compact" min-width="200">
        <v-list-subheader>Export entire vault</v-list-subheader>
        <v-list-item prepend-icon="mdi-folder-zip-outline" title="Download as ZIP" @click="exportVaultZip" />
        <v-list-item prepend-icon="mdi-archive-arrow-down-outline" title="Download as tar.gz" @click="exportVaultTar" />
      </v-list>
    </v-menu>
    <v-btn
      icon="mdi-dice-5-outline"
      size="small"
      density="compact"
      title="Open random note"
      @click="openRandomNote"
    />
    <v-btn
      icon="mdi-calendar-today"
      size="small"
      density="compact"
      title="Open daily note"
      @click="openDailyNote"
    />
    <v-spacer />
    <v-btn
      icon="mdi-sort-alphabetical-ascending"
      size="small"
      density="compact"
      title="Sort A→Z"
      @click="sort = 'asc'"
      :color="sort === 'asc' ? 'primary' : undefined"
    />
    <v-btn
      icon="mdi-sort-alphabetical-descending"
      size="small"
      density="compact"
      title="Sort Z→A"
      @click="sort = 'desc'"
      :color="sort === 'desc' ? 'primary' : undefined"
    />
  </div>

  <!-- New entity dialog -->
  <NewEntityDialog
    v-model="newEntityDialog"
    :initial-type-id="newEntityDialogInitialTypeId"
    :initial-file-name="newEntityDialogInitialFileName"
  />

  <!-- New note dialog -->
  <v-dialog v-model="newNoteDialog" max-width="400">
    <v-card>
      <v-card-title>New Note</v-card-title>
      <v-card-text>
        <v-text-field
          v-model="newNoteName"
          label="File name"
          placeholder="note.md"
          autofocus
          @keyup.enter="confirmNewNote"
        />
        <v-select
          v-if="loadingNewNoteTemplates || noteTemplateItems.length > 1"
          v-model="newNoteTemplateId"
          :items="noteTemplateItems"
          item-title="title"
          item-value="value"
          label="Template"
          prepend-inner-icon="mdi-shape-outline"
          density="comfortable"
          :loading="loadingNewNoteTemplates"
          hint="Choose a regular note or start from an entity template."
          persistent-hint
        />
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn @click="newNoteDialog = false">Cancel</v-btn>
        <v-btn color="primary" @click="confirmNewNote">{{ newNoteActionLabel }}</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>

  <!-- New folder dialog -->
  <v-dialog v-model="newFolderDialog" max-width="400">
    <v-card>
      <v-card-title>New Folder</v-card-title>
      <v-card-text>
        <v-text-field
          v-model="newFolderName"
          label="Folder name"
          autofocus
          @keyup.enter="confirmNewFolder"
        />
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn @click="newFolderDialog = false">Cancel</v-btn>
        <v-btn color="primary" @click="confirmNewFolder">Create</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, provide, ref } from 'vue';
import { apiListEntityTypes } from '@/api/client';
import type { EntityTypeSchema } from '@/api/types';
import { useVaultsStore } from '@/stores/vaults';
import { useFilesStore } from '@/stores/files';
import { useTabsStore } from '@/stores/tabs';
import { useUiStore } from '@/stores/ui';
import NewEntityDialog from '@/components/modals/NewEntityDialog.vue';

const vaultsStore = useVaultsStore();
const filesStore = useFilesStore();
const tabsStore = useTabsStore();
const uiStore = useUiStore();

const sort = ref<'asc' | 'desc'>('asc');
const newNoteDialog = ref(false);
const newNoteName = ref('');
const newNoteTemplateId = ref('');
const loadingNewNoteTemplates = ref(false);
const entityTypes = ref<EntityTypeSchema[]>([]);
const newFolderDialog = ref(false);
const newFolderName = ref('');
const newEntityDialog = ref(false);
const newEntityDialogInitialTypeId = ref<string | null>(null);
const newEntityDialogInitialFileName = ref('');

const noteTemplateItems = computed(() => [
  { title: 'Regular note', value: '' },
  ...entityTypes.value.map((type) => ({
    title: `${type.name} entity`,
    value: type.id,
  })),
]);

const selectedNoteEntityType = computed(() =>
  entityTypes.value.find((type) => type.id === newNoteTemplateId.value) ?? null,
);

const newNoteActionLabel = computed(() =>
  selectedNoteEntityType.value ? 'Continue' : 'Create',
);

function openGraph() {
  const vaultId = vaultsStore.activeVaultId;
  if (vaultId) tabsStore.openGraphTab(tabsStore.activePaneId, vaultId);
}

provide('fileTreeSort', sort);

function newNote() {
  newNoteName.value = '';
  newNoteTemplateId.value = '';
  newNoteDialog.value = true;
  void loadEntityTypes();
}

async function confirmNewNote() {
  const vaultId = vaultsStore.activeVaultId;
  if (!vaultId || !newNoteName.value.trim()) return;

  if (selectedNoteEntityType.value) {
    newEntityDialogInitialTypeId.value = selectedNoteEntityType.value.id;
    newEntityDialogInitialFileName.value = newNoteName.value.trim();
    newNoteDialog.value = false;
    newEntityDialog.value = true;
    return;
  }

  const name = newNoteName.value.trim().endsWith('.md')
    ? newNoteName.value.trim()
    : newNoteName.value.trim() + '.md';
  newNoteDialog.value = false;
  const node = await filesStore.createFile(vaultId, name);
  if (node) {
    tabsStore.openTab(tabsStore.activePaneId, node.path, node.path.split('/').pop()!);
  }
}

async function loadEntityTypes() {
  loadingNewNoteTemplates.value = true;
  try {
    const result = await apiListEntityTypes();
    entityTypes.value = result.entity_types ?? [];
  } catch {
    entityTypes.value = [];
  } finally {
    loadingNewNoteTemplates.value = false;
  }
}

function newFolder() {
  newFolderName.value = '';
  newFolderDialog.value = true;
}

async function confirmNewFolder() {
  const vaultId = vaultsStore.activeVaultId;
  if (!vaultId || !newFolderName.value.trim()) return;
  newFolderDialog.value = false;
  await filesStore.createDirectory(vaultId, newFolderName.value.trim());
}

async function refresh() {
  const vaultId = vaultsStore.activeVaultId;
  if (vaultId) await filesStore.loadTree(vaultId);
}

async function openRandomNote() {
  const vaultId = vaultsStore.activeVaultId;
  if (!vaultId) return;
  const path = await filesStore.getRandomNote(vaultId);
  tabsStore.openTab(tabsStore.activePaneId, path, path.split('/').pop()!);
}

async function openDailyNote() {
  const vaultId = vaultsStore.activeVaultId;
  if (!vaultId) return;
  const note = await filesStore.getDailyNote(vaultId);
  tabsStore.openTab(tabsStore.activePaneId, note.path, note.path.split('/').pop()!);
}

async function exportVaultZip() {
  const vaultId = vaultsStore.activeVaultId;
  if (!vaultId) return;
  // Export all top-level entries by passing each root tree node path.
  const paths = filesStore.tree.map((n) => n.path);
  if (paths.length === 0) return;
  await filesStore.downloadAsZip(vaultId, paths);
}

async function exportVaultTar() {
  const vaultId = vaultsStore.activeVaultId;
  if (!vaultId) return;
  const paths = filesStore.tree.map((n) => n.path);
  if (paths.length === 0) return;
  await filesStore.downloadAsTar(vaultId, paths);
}
</script>

<style scoped>
.sidebar-actions {
  flex-shrink: 0;
}
</style>
