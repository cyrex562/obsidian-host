<template>
  <v-app-bar
    height="40"
    flat
    style="background: rgb(var(--v-theme-surface)); border-bottom: 1px solid rgb(var(--v-theme-border));"
  >
    <v-app-bar-title class="text-caption font-weight-medium" style="color: rgb(var(--v-theme-on-background));">
      {{ vaultsStore.getActive()?.name ?? 'Obsidian Host' }}
    </v-app-bar-title>

    <div class="d-flex align-center ga-2 mr-2">
      <v-chip size="x-small" :color="wsConnected ? 'success' : 'warning'" variant="tonal">
        <v-icon start :icon="wsConnected ? 'mdi-lan-connect' : 'mdi-lan-disconnect'" />
        {{ wsConnected ? 'Connected' : 'Offline' }}
      </v-chip>
      <v-chip size="x-small" :color="dirtyCount > 0 ? 'warning' : 'success'" variant="tonal">
        <v-icon start :icon="dirtyCount > 0 ? 'mdi-content-save-alert-outline' : 'mdi-content-save-check-outline'" />
        {{ dirtyCount > 0 ? `${dirtyCount} unsaved` : 'Saved' }}
      </v-chip>
    </div>

    <template #append>
      <v-btn
        icon="mdi-magnify"
        size="small"
        density="compact"
        title="Search (Ctrl+Shift+F)"
        @click="emit('open-search')"
      />
      <v-btn
        icon="mdi-puzzle-outline"
        size="small"
        density="compact"
        title="Plugins"
        @click="emit('open-plugins')"
      />
      <v-btn
        icon="mdi-help-circle-outline"
        size="small"
        density="compact"
        title="Theme"
        @click="toggleTheme"
      />

      <v-menu>
        <template #activator="{ props }">
          <v-btn
            v-bind="props"
            size="small"
            variant="text"
            prepend-icon="mdi-account-circle-outline"
          >
            {{ username }}
          </v-btn>
        </template>

        <v-list density="compact" min-width="220">
          <v-list-item
            prepend-icon="mdi-lock-reset"
            title="Change password"
            @click="goToChangePassword"
          />
          <v-list-item
            v-if="authStore.isAdmin"
            prepend-icon="mdi-account-multiple-plus-outline"
            title="Manage users"
            @click="goToAdminUsers"
          />
          <v-divider class="my-1" />
          <v-list-item
            prepend-icon="mdi-logout"
            title="Sign out"
            @click="signOut"
          />
        </v-list>
      </v-menu>
    </template>
  </v-app-bar>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useRouter } from 'vue-router';
import { useVaultsStore } from '@/stores/vaults';
import { usePreferencesStore } from '@/stores/preferences';
import { useTabsStore } from '@/stores/tabs';
import { useAuthStore } from '@/stores/auth';
import { useWebSocket } from '@/composables/useWebSocket';

const emit = defineEmits<{
  'open-search': [];
  'open-plugins': [];
}>();

const vaultsStore = useVaultsStore();
const prefsStore = usePreferencesStore();
const tabsStore = useTabsStore();
const authStore = useAuthStore();
const router = useRouter();
const { connected } = useWebSocket();

const dirtyCount = computed(() => tabsStore.dirtyTabs.length);
const wsConnected = computed(() => connected.value);
const username = computed(() => authStore.profile?.username ?? 'Account');

function toggleTheme() {
  prefsStore.set('theme', prefsStore.prefs.theme === 'dark' ? 'light' : 'dark');
  prefsStore.save();
}

function goToChangePassword() {
  void router.push('/change-password');
}

function goToAdminUsers() {
  void router.push('/admin/users');
}

async function signOut() {
  await authStore.logout();
  await router.replace('/login');
}
</script>

