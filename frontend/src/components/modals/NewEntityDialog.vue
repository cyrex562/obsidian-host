<template>
  <v-dialog v-model="model" max-width="500" @after-leave="reset">
    <v-card>
      <v-card-title class="d-flex align-center gap-2">
        <v-icon>mdi-cube-outline</v-icon>
        New Entity
      </v-card-title>
      <v-card-text>
        <div v-if="loading" class="d-flex justify-center py-4">
          <v-progress-circular indeterminate />
        </div>

        <template v-else>
          <!-- Entity type picker -->
          <v-select
            v-model="selectedTypeId"
            :items="typeItems"
            item-title="name"
            item-value="id"
            label="Entity type"
            prepend-inner-icon="mdi-shape-outline"
            density="comfortable"
            class="mb-3"
            @update:model-value="onTypeChange"
          >
            <template #item="{ item, props: listProps }">
              <v-list-item v-bind="listProps">
                <template #prepend>
                  <v-icon :color="item.raw.color || undefined">{{ item.raw.icon || 'mdi-cube-outline' }}</v-icon>
                </template>
              </v-list-item>
            </template>
          </v-select>

          <!-- File name -->
          <v-text-field
            v-model="fileName"
            label="File name"
            placeholder="my-entity.md"
            density="comfortable"
            class="mb-2"
            @keyup.enter="create"
          />

          <!-- Optional folder -->
          <v-text-field
            v-model="folder"
            label="Folder (optional)"
            placeholder="World/Characters"
            density="comfortable"
            hint="Leave blank to place in vault root"
            persistent-hint
          />

          <!-- show_on_create fields -->
          <template v-if="createFields.length">
            <v-divider class="my-3" />
            <div class="text-caption text-medium-emphasis mb-2">Quick fields</div>
            <template v-for="field in createFields" :key="field.key">
              <v-select
                v-if="field.field_type === 'enum'"
                v-model="quickValues[field.key]"
                :items="field.values"
                :label="field.label"
                density="comfortable"
                class="mb-2"
              />
              <v-textarea
                v-else-if="field.field_type === 'text'"
                v-model="quickValues[field.key]"
                :label="field.label"
                density="comfortable"
                rows="2"
                auto-grow
                class="mb-2"
              />
              <v-text-field
                v-else
                v-model="quickValues[field.key]"
                :label="field.label"
                density="comfortable"
                class="mb-2"
              />
            </template>
          </template>
        </template>
      </v-card-text>

      <v-card-actions>
        <v-spacer />
        <v-btn @click="model = false">Cancel</v-btn>
        <v-btn
          color="primary"
          :disabled="!canCreate"
          :loading="creating"
          @click="create"
        >
          Create &amp; Open
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { parse as parseYaml, stringify as stringifyYaml } from 'yaml';
import { apiListEntityTypes, apiGetEntityTypeTemplate } from '@/api/client';
import type { EntityTypeSchema, FieldSchema } from '@/api/types';
import { useVaultsStore } from '@/stores/vaults';
import { useFilesStore } from '@/stores/files';
import { useTabsStore } from '@/stores/tabs';
import { useEditorStore } from '@/stores/editor';

const props = withDefaults(defineProps<{ initialTypeId?: string | null; initialFileName?: string }>(), {
    initialTypeId: null,
    initialFileName: '',
});
const model = defineModel<boolean>({ default: false });
const emit = defineEmits<{ created: [path: string] }>();

const vaultsStore = useVaultsStore();
const filesStore = useFilesStore();
const tabsStore = useTabsStore();
const editorStore = useEditorStore();

const loading = ref(false);
const creating = ref(false);
const entityTypes = ref<EntityTypeSchema[]>([]);
const selectedTypeId = ref<string | null>(null);
const fileName = ref('');
const folder = ref('');
const quickValues = ref<Record<string, string>>({});

const typeItems = computed(() => entityTypes.value);

const selectedType = computed(() =>
    entityTypes.value.find((t) => t.id === selectedTypeId.value) ?? null,
);

const createFields = computed<FieldSchema[]>(() => {
    if (!selectedType.value) return [];
    const keys = selectedType.value.show_on_create ?? [];
    return selectedType.value.fields.filter((f) => keys.includes(f.key));
});

const canCreate = computed(
    () => !!selectedTypeId.value && !!fileName.value.trim(),
);

watch(model, (open) => {
    if (open) {
        applyInitialFileName();
        if (entityTypes.value.length === 0) {
            void loadTypes();
        } else {
            applyInitialTypeSelection();
        }
    }
});

watch(() => props.initialTypeId, () => {
    if (model.value) {
        applyInitialTypeSelection();
    }
});

watch(() => props.initialFileName, () => {
    if (model.value) {
        applyInitialFileName();
    }
});

async function loadTypes() {
    loading.value = true;
    try {
        const result = await apiListEntityTypes();
        entityTypes.value = result.entity_types ?? [];
        applyInitialTypeSelection();
    } catch {
        // non-critical — show empty list
    } finally {
        loading.value = false;
    }
}

function onTypeChange() {
    quickValues.value = defaultQuickValuesForSelectedType();
}

function applyInitialTypeSelection() {
    if (props.initialTypeId) {
        const matchingType = entityTypes.value.find((type) => type.id === props.initialTypeId);
        if (matchingType) {
            selectedTypeId.value = matchingType.id;
            quickValues.value = defaultQuickValuesForType(matchingType);
            return;
        }
    }

    if (!selectedTypeId.value && entityTypes.value.length === 1) {
        selectedTypeId.value = entityTypes.value[0].id;
    }

    quickValues.value = defaultQuickValuesForSelectedType();
}

function defaultQuickValuesForSelectedType(): Record<string, string> {
    return defaultQuickValuesForType(selectedType.value);
}

function defaultQuickValuesForType(type: EntityTypeSchema | null): Record<string, string> {
    if (!type) return {};
    const createFieldKeys = new Set(type.show_on_create ?? []);
    return type.fields.reduce<Record<string, string>>((acc, field) => {
        if (!createFieldKeys.has(field.key) || field.default === undefined || field.default === null) {
            return acc;
        }
        acc[field.key] = String(field.default);
        return acc;
    }, {});
}

function applyInitialFileName() {
    if (props.initialFileName) {
        fileName.value = props.initialFileName;
    }
}

async function create() {
    if (!canCreate.value) return;
    const vaultId = vaultsStore.activeVaultId;
    if (!vaultId) return;

    const typeId = selectedTypeId.value!;
    const type = selectedType.value;
    let name = fileName.value.trim();
    if (!name.endsWith('.md')) name += '.md';

    const dir = folder.value.trim().replace(/\/+$/, '');
    const path = dir ? `${dir}/${name}` : name;

    creating.value = true;
    try {
        // Fetch the entity template to get correct frontmatter skeleton
        let templateContent = '';
        try {
            const tmpl = await apiGetEntityTypeTemplate(vaultId, typeId);
            templateContent = tmpl.content ?? '';
        } catch {
            // fallback: build minimal frontmatter
        }

        // Patch in quick-field values and set title from filename
        const title = name.replace(/\.md$/, '');
        const content = patchTemplate(templateContent, typeId, type, title);

        const node = await filesStore.createFile(vaultId, path, content);
        if (node) {
            model.value = false;
            // Open in structural editor mode
            editorStore.setMode('structural');
            tabsStore.openTab(tabsStore.activePaneId, node.path, node.path.split('/').pop()!);
            emit('created', node.path);
        }
    } finally {
        creating.value = false;
    }
}

function patchTemplate(
    template: string,
    typeId: string,
    type: EntityTypeSchema | null,
    title: string,
): string {
    // Parse existing YAML frontmatter (if any)
    let fm: Record<string, unknown> = {};
    let body = template;

    const match = template.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/);
    if (match) {
        try {
            fm = parseYaml(match[1]) ?? {};
        } catch {
            fm = {};
        }
        body = match[2] ?? '';
    }

    // Set required codex keys
    fm.codex_type = typeId;
    if (type) {
        const plugin_id = type.plugin_id;
        fm.codex_plugin = plugin_id;
        if (type.labels?.length) {
            fm.codex_labels = type.labels;
        }
        // Set display field / title
        const displayKey = type.display_field ?? 'name';
        if (!(displayKey in fm)) fm[displayKey] = title;
    }

    // Apply quick-field values
    for (const [key, val] of Object.entries(quickValues.value)) {
        if (val.trim()) fm[key] = val.trim();
    }

    return `---\n${stringifyYaml(fm)}---\n${body}`;
}

function reset() {
    selectedTypeId.value = null;
    fileName.value = '';
    folder.value = '';
    quickValues.value = {};
}
</script>
