<template>
  <v-container class="fill-height d-flex align-center justify-center">
    <v-card min-width="360" max-width="420">
      <v-card-title class="text-center pa-6">
        <v-icon icon="mdi-notebook-outline" size="40" color="primary" />
        <div class="mt-2 text-h6">Obsidian Host</div>
      </v-card-title>

      <v-card-text>
        <v-alert v-if="error" type="error" class="mb-4" closable @click:close="error = ''">{{ error }}</v-alert>

        <v-text-field
          v-model="username"
          label="Username"
          prepend-inner-icon="mdi-account-outline"
          autofocus
          @keyup.enter="login"
        />
        <v-text-field
          v-model="password"
          label="Password"
          type="password"
          prepend-inner-icon="mdi-lock-outline"
          @keyup.enter="login"
        />
      </v-card-text>

      <v-card-actions class="px-4 pb-4">
        <v-btn block color="primary" :loading="loading" @click="login">Sign In</v-btn>
      </v-card-actions>
    </v-card>
  </v-container>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useRouter } from 'vue-router';
import { useAuthStore } from '@/stores/auth';

const router = useRouter();
const authStore = useAuthStore();

const username = ref('');
const password = ref('');
const loading = ref(false);
const error = ref('');

async function login() {
  if (!username.value || !password.value) return;
  loading.value = true;
  error.value = '';
  try {
    await authStore.login(username.value, password.value);
    const redirect = typeof router.currentRoute.value.query.redirect === 'string'
      ? router.currentRoute.value.query.redirect
      : '/';
    router.push(redirect);
  } catch (e: any) {
    error.value = e?.message ?? 'Login failed.';
  } finally {
    loading.value = false;
  }
}
</script>
