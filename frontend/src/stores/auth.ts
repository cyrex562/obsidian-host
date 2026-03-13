import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { apiLogin, apiRefreshToken, apiLogout, apiMe, apiChangePassword } from '@/api/client';
import type { LoginResponse, AuthenticatedUserProfile } from '@/api/types';

const ACCESS_TOKEN_KEY = 'obsidian_access_token';
const REFRESH_TOKEN_KEY = 'obsidian_refresh_token';
const EXPIRES_AT_KEY = 'obsidian_token_expires_at';

export const useAuthStore = defineStore('auth', () => {
    const accessToken = ref<string | null>(localStorage.getItem(ACCESS_TOKEN_KEY));
    const refreshToken = ref<string | null>(localStorage.getItem(REFRESH_TOKEN_KEY));
    const expiresAt = ref<number>(parseInt(localStorage.getItem(EXPIRES_AT_KEY) ?? '0', 10));
    const profile = ref<AuthenticatedUserProfile | null>(null);
    const loadingProfile = ref(false);

    const isAuthenticated = computed(() => !!accessToken.value);
    const isExpired = computed(() => Date.now() > expiresAt.value - 60_000); // 60s margin
    const isAdmin = computed(() => !!profile.value?.is_admin);
    const mustChangePassword = computed(() => !!profile.value?.must_change_password);

    function _applyTokens(resp: LoginResponse) {
        accessToken.value = resp.access_token;
        refreshToken.value = resp.refresh_token;
        expiresAt.value = Date.now() + resp.expires_in * 1000;
        localStorage.setItem(ACCESS_TOKEN_KEY, resp.access_token);
        localStorage.setItem(REFRESH_TOKEN_KEY, resp.refresh_token);
        localStorage.setItem(EXPIRES_AT_KEY, String(expiresAt.value));
    }

    async function login(username: string, password: string) {
        const resp = await apiLogin(username, password);
        _applyTokens(resp);
        await loadProfile(true);
    }

    async function refresh() {
        if (!refreshToken.value) throw new Error('No refresh token');
        const resp = await apiRefreshToken(refreshToken.value);
        _applyTokens(resp);
    }

    async function logout() {
        try { await apiLogout(); } catch { /* ignore server errors on logout */ }
        accessToken.value = null;
        refreshToken.value = null;
        expiresAt.value = 0;
        profile.value = null;
        localStorage.removeItem(ACCESS_TOKEN_KEY);
        localStorage.removeItem(REFRESH_TOKEN_KEY);
        localStorage.removeItem(EXPIRES_AT_KEY);
    }

    async function loadProfile(force = false) {
        if (!accessToken.value) {
            profile.value = null;
            return null;
        }
        if (!force && profile.value) return profile.value;

        loadingProfile.value = true;
        try {
            profile.value = await apiMe();
            return profile.value;
        } finally {
            loadingProfile.value = false;
        }
    }

    // Call before any authenticated request to ensure the token is still valid.
    async function ensureFresh() {
        if (accessToken.value && isExpired.value) {
            await refresh();
        }
    }

    async function changePassword(currentPassword: string, newPassword: string) {
        await apiChangePassword({
            current_password: currentPassword,
            new_password: newPassword,
        });
        await loadProfile(true);
    }

    return {
        accessToken,
        refreshToken,
        expiresAt,
        profile,
        loadingProfile,
        isAuthenticated,
        isExpired,
        isAdmin,
        mustChangePassword,
        login,
        refresh,
        logout,
        ensureFresh,
        loadProfile,
        changePassword,
    };
});
