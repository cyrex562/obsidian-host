<template>
  <v-main class="d-flex align-center justify-center" style="min-height: 100vh;">
    <v-card width="520" class="pa-4">
      <v-card-title class="text-h6">Change your password</v-card-title>
      <v-card-subtitle>
        Your account requires a password update before you can continue.
      </v-card-subtitle>

      <v-card-text class="pt-4">
        <v-alert v-if="error" type="error" variant="tonal" class="mb-3">{{ error }}</v-alert>
        <v-alert v-if="success" type="success" variant="tonal" class="mb-3">{{ success }}</v-alert>

        <v-text-field
          v-model="currentPassword"
          label="Current password"
          type="password"
          autocomplete="current-password"
          density="comfortable"
        />

        <v-text-field
          v-model="newPassword"
          label="New password"
          type="password"
          autocomplete="new-password"
          hint="Minimum 12 characters"
          persistent-hint
          density="comfortable"
        />

        <v-text-field
          v-model="confirmPassword"
          label="Confirm new password"
          type="password"
          autocomplete="new-password"
          density="comfortable"
        />
      </v-card-text>

      <v-card-actions class="justify-space-between">
        <v-btn variant="text" color="error" @click="logout" :disabled="saving">Sign out</v-btn>
        <v-btn color="primary" :loading="saving" @click="submit">Update password</v-btn>
      </v-card-actions>
    </v-card>
  </v-main>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useRouter } from 'vue-router';
import { useAuthStore } from '@/stores/auth';

const authStore = useAuthStore();
const router = useRouter();

const currentPassword = ref('');
const newPassword = ref('');
const confirmPassword = ref('');
const saving = ref(false);
const error = ref('');
const success = ref('');

async function submit() {
  error.value = '';
  success.value = '';

  if (!currentPassword.value || !newPassword.value || !confirmPassword.value) {
    error.value = 'Please fill in all password fields.';
    return;
  }

  if (newPassword.value.length < 12) {
    error.value = 'New password must be at least 12 characters.';
    return;
  }

  if (newPassword.value !== confirmPassword.value) {
    error.value = 'New password and confirmation do not match.';
    return;
  }

  saving.value = true;
  try {
    await authStore.changePassword(currentPassword.value, newPassword.value);
    success.value = 'Password updated successfully.';

    const redirect = typeof router.currentRoute.value.query.redirect === 'string'
      ? router.currentRoute.value.query.redirect
      : '/';

    await router.replace(redirect);
  } catch (e: any) {
    error.value = e?.message ?? 'Failed to change password.';
  } finally {
    saving.value = false;
  }
}

async function logout() {
  await authStore.logout();
  await router.replace('/login');
}
</script>
