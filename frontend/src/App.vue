<template>
  <v-app :theme="theme">
    <router-view />
  </v-app>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { usePreferencesStore } from '@/stores/preferences';
import { useAuthStore } from '@/stores/auth';

const prefsStore = usePreferencesStore();
const authStore = useAuthStore();
const router = useRouter();

// Vuetify theme name driven by user preference
const theme = computed(() =>
  prefsStore.prefs.theme === 'dark' ? 'obsidianDark' : 'obsidianLight',
);

// Bootstrap: load preferences, then open WS
onMounted(async () => {
  const isLoginRoute = router.currentRoute.value.path === '/login';

  if (authStore.isAuthenticated || !isLoginRoute) {
    await prefsStore.load();
  }

  if (authStore.isAuthenticated) {
    try {
      await authStore.ensureFresh();
      await authStore.loadProfile();
    } catch {
      await authStore.logout();
      if (router.currentRoute.value.path !== '/login') {
        await router.replace({
          path: '/login',
          query: { redirect: router.currentRoute.value.fullPath || '/' },
        });
      }
    }
  }
});
</script>

<style>
:root {
  --bg-primary: #111111;
  --bg-secondary: #0a0a0a;
  --bg-tertiary: #2a2a2a;
  --text-primary: #e5e7eb;
  --text-secondary: #9ca3af;
  --border-color: #27272a;
  --accent-color: #8b5cf6;
  --accent-hover: #a78bfa;
  --error-color: #ef4444;
}

/* Global resets — keep Obsidian feel inside Vuetify */
html, body {
  overflow: hidden;
  height: 100vh;
}

* {
  box-sizing: border-box;
}

body {
  background: var(--bg-primary);
  color: var(--text-primary);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

#app {
  height: 100vh;
}

/* Monospace for editor areas */
.mono {
  font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
}

.text-secondary {
  color: var(--text-secondary) !important;
}

/* Rendered markdown content */
.markdown-body h1, .markdown-body h2, .markdown-body h3 {
  margin: 0.75em 0 0.4em;
  font-weight: 600;
}
.markdown-body p { margin-bottom: 0.8em; }
.markdown-body code {
  background: rgba(139, 92, 246, 0.12);
  border-radius: 3px;
  padding: 0.1em 0.35em;
  font-size: 0.88em;
}
.markdown-body pre code {
  background: none;
  padding: 0;
}
.markdown-body pre {
  background: #0a0a0a;
  border: 1px solid #27272a;
  border-radius: 6px;
  padding: 1em;
  overflow-x: auto;
  margin-bottom: 1em;
}
.markdown-body blockquote {
  border-left: 3px solid #8b5cf6;
  margin: 0.5em 0;
  padding: 0.25em 1em;
  color: #9ca3af;
}
.markdown-body a {
  color: #8b5cf6;
  text-decoration: none;
}
.markdown-body a:hover {
  color: #a78bfa;
  text-decoration: underline;
}
.markdown-body table { border-collapse: collapse; width: 100%; margin-bottom: 1em; }
.markdown-body th, .markdown-body td {
  border: 1px solid #27272a;
  padding: 0.5em 0.75em;
}
.markdown-body th { background: #1a1a1a; }
</style>
