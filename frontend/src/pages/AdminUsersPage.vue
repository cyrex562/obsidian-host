<template>
  <v-main class="pa-6" style="min-height: 100vh; background: rgb(var(--v-theme-background));">
    <div class="d-flex justify-space-between align-center mb-4">
      <div>
        <h1 class="text-h5 mb-1">Admin · User Management</h1>
        <p class="text-body-2 text-medium-emphasis">Create accounts with temporary passwords and admin role flags.</p>
      </div>
      <div class="d-flex ga-2">
        <v-btn variant="outlined" @click="goHome">Back to app</v-btn>
        <v-btn color="primary" @click="loadUsers" :loading="loading">Refresh</v-btn>
      </div>
    </div>

    <v-alert v-if="error" type="error" variant="tonal" class="mb-3">{{ error }}</v-alert>
    <v-alert v-if="createdTempPassword" type="info" variant="tonal" class="mb-3">
      User created. Temporary password for <strong>{{ createdUsername }}</strong>:<br />
      <code>{{ createdTempPassword }}</code>
    </v-alert>

    <v-card class="mb-6">
      <v-card-title class="text-subtitle-1">Create new user</v-card-title>
      <v-card-text>
        <v-row>
          <v-col cols="12" md="4">
            <v-text-field v-model="newUsername" label="Username" density="comfortable" />
          </v-col>
          <v-col cols="12" md="4">
            <v-text-field
              v-model="newTemporaryPassword"
              label="Temporary password (optional)"
              density="comfortable"
              hint="Leave blank to auto-generate"
              persistent-hint
            />
          </v-col>
          <v-col cols="12" md="2" class="d-flex align-center">
            <v-checkbox v-model="newIsAdmin" label="Admin" hide-details />
          </v-col>
          <v-col cols="12" md="2" class="d-flex align-center justify-end">
            <v-btn color="primary" :loading="creating" @click="createUser">Create user</v-btn>
          </v-col>
        </v-row>
      </v-card-text>
    </v-card>

    <v-card>
      <v-data-table
        :headers="headers"
        :items="users"
        :loading="loading"
        item-key="id"
        density="comfortable"
      >
        <template #item.is_admin="{ item }">
          <v-chip :color="item.is_admin ? 'primary' : 'default'" size="small" variant="tonal">
            {{ item.is_admin ? 'Admin' : 'User' }}
          </v-chip>
        </template>

        <template #item.must_change_password="{ item }">
          <v-chip :color="item.must_change_password ? 'warning' : 'success'" size="small" variant="tonal">
            {{ item.must_change_password ? 'Required' : 'No' }}
          </v-chip>
        </template>

        <template #item.created_at="{ item }">
          {{ formatDate(item.created_at) }}
        </template>
      </v-data-table>
    </v-card>
  </v-main>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { apiCreateUser, apiListUsers } from '@/api/client';
import type { AdminUser } from '@/api/types';

const router = useRouter();

const users = ref<AdminUser[]>([]);
const loading = ref(false);
const creating = ref(false);
const error = ref('');

const newUsername = ref('');
const newTemporaryPassword = ref('');
const newIsAdmin = ref(false);

const createdTempPassword = ref('');
const createdUsername = ref('');

const headers = [
  { title: 'Username', key: 'username' },
  { title: 'Role', key: 'is_admin' },
  { title: 'Must change password', key: 'must_change_password' },
  { title: 'Created', key: 'created_at' },
] as const;

onMounted(() => {
  void loadUsers();
});

async function loadUsers() {
  loading.value = true;
  error.value = '';
  try {
    users.value = await apiListUsers();
  } catch (e: any) {
    error.value = e?.message ?? 'Failed to load users.';
  } finally {
    loading.value = false;
  }
}

async function createUser() {
  error.value = '';
  createdTempPassword.value = '';
  createdUsername.value = '';

  if (!newUsername.value.trim()) {
    error.value = 'Username is required.';
    return;
  }

  creating.value = true;
  try {
    const created = await apiCreateUser({
      username: newUsername.value.trim(),
      temporary_password: newTemporaryPassword.value.trim() || undefined,
      is_admin: newIsAdmin.value,
    });

    createdUsername.value = created.username;
    createdTempPassword.value = created.temporary_password;

    newUsername.value = '';
    newTemporaryPassword.value = '';
    newIsAdmin.value = false;

    await loadUsers();
  } catch (e: any) {
    error.value = e?.message ?? 'Failed to create user.';
  } finally {
    creating.value = false;
  }
}

function formatDate(value: string) {
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return value;
  return d.toLocaleString();
}

function goHome() {
  void router.push('/');
}
</script>
