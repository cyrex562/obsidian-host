<template>
  <v-dialog :model-value="modelValue" max-width="560" @update:model-value="emit('update:modelValue', $event)">
    <v-card>
      <v-card-title class="d-flex align-center">
        Plugins
        <v-spacer />
        <v-btn icon="mdi-close" size="small" variant="plain" @click="close" />
      </v-card-title>

      <v-card-text style="max-height: 480px; overflow-y: auto;">
        <v-progress-linear v-if="loading" indeterminate class="mb-2" />

        <div v-if="worldbuildingTypes.length" class="mb-4">
          <div class="text-subtitle-2 mb-2">Worldbuilding</div>
          <div class="d-flex flex-wrap ga-2">
            <v-btn
              v-for="entityType in worldbuildingTypes"
              :key="entityType.id"
              variant="tonal"
              color="primary"
              :text="`New ${entityType.name}`"
              @click="openNewEntityDialog(entityType.id)"
            />
          </div>
          <div class="text-caption text-medium-emphasis mt-2">
            Create characters, places, factions, and other entities without editing frontmatter manually.
          </div>
        </div>

        <v-list density="compact">
          <v-list-item v-for="plugin in plugins" :key="plugin.id">
            <template #prepend>
              <v-switch
                :model-value="plugin.enabled"
                hide-details
                density="compact"
                @update:model-value="toggle(plugin.id, !plugin.enabled)"
              />
            </template>
            <v-list-item-title>{{ plugin.name }}</v-list-item-title>
            <v-list-item-subtitle>
              {{ plugin.description || `${plugin.id} · v${plugin.version}` }}
            </v-list-item-subtitle>
          </v-list-item>
        </v-list>

        <p v-if="!loading && plugins.length === 0" class="text-caption text-secondary text-center">
          No plugins installed.
        </p>
      </v-card-text>

      <v-card-actions>
        <v-spacer />
        <v-btn @click="close">Close</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>

  <NewEntityDialog v-model="newEntityDialogOpen" :initial-type-id="selectedEntityTypeId" />
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { apiListEntityTypes, apiListPlugins, apiTogglePlugin } from '@/api/client';
import type { EntityTypeSchema } from '@/api/types';
import NewEntityDialog from './NewEntityDialog.vue';

interface Plugin {
  id: string;
  name: string;
  description: string;
  version: string;
  enabled: boolean;
}

interface PluginApiItem {
  id?: string;
  name?: string;
  description?: string | null;
  version?: string;
  enabled?: boolean;
  manifest?: {
    id?: string;
    name?: string;
    description?: string | null;
    version?: string;
  };
}

const props = defineProps<{ modelValue: boolean }>();
const emit = defineEmits<{ 'update:modelValue': [v: boolean] }>();

const plugins = ref<Plugin[]>([]);
const entityTypes = ref<EntityTypeSchema[]>([]);
const loading = ref(false);
const newEntityDialogOpen = ref(false);
const selectedEntityTypeId = ref<string | null>(null);

const worldbuildingTypes = computed(() =>
  entityTypes.value.filter((entityType) => entityType.plugin_id.toLowerCase().includes('worldbuilding')),
);

watch(() => props.modelValue, async (open) => {
  if (open) await load();
});

async function load() {
  loading.value = true;
  try {
     const [pluginsResult, entityTypesResult] = await Promise.allSettled([
       apiListPlugins(),
       apiListEntityTypes(),
     ]);

     plugins.value = pluginsResult.status === 'fulfilled'
       ? normalizePlugins(pluginsResult.value.plugins ?? [])
       : [];
     entityTypes.value = entityTypesResult.status === 'fulfilled'
       ? (entityTypesResult.value.entity_types ?? [])
       : [];
  } finally {
    loading.value = false;
  }
}

async function toggle(id: string, enable: boolean) {
  await apiTogglePlugin(id, enable);
  const p = plugins.value.find(pl => pl.id === id);
  if (p) p.enabled = enable;
}

function close() {
  emit('update:modelValue', false);
}

async function openNewEntityDialog(typeId: string) {
  selectedEntityTypeId.value = typeId;
  close();
  await nextTick();
  newEntityDialogOpen.value = true;
}

function normalizePlugins(rawPlugins: unknown[]): Plugin[] {
  return rawPlugins
    .map((item) => normalizePlugin(item as PluginApiItem))
    .filter((plugin): plugin is Plugin => plugin !== null);
}

function normalizePlugin(plugin: PluginApiItem): Plugin | null {
  const manifest = plugin.manifest ?? {};
  const id = plugin.id ?? manifest.id;
  const name = plugin.name ?? manifest.name;

  if (!id || !name) {
    return null;
  }

  return {
    id,
    name,
    description: plugin.description ?? manifest.description ?? '',
    version: plugin.version ?? manifest.version ?? '0.0.0',
    enabled: Boolean(plugin.enabled),
  };
}
</script>
