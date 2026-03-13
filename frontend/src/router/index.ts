import { createRouter, createWebHistory } from 'vue-router';
import { useAuthStore } from '@/stores/auth';

const router = createRouter({
    history: createWebHistory(),
    routes: [
        {
            path: '/login',
            name: 'login',
            component: () => import('@/pages/LoginPage.vue'),
            meta: { public: true },
        },
        {
            path: '/change-password',
            name: 'change-password',
            component: () => import('@/pages/ChangePasswordPage.vue'),
        },
        {
            path: '/admin/users',
            name: 'admin-users',
            component: () => import('@/pages/AdminUsersPage.vue'),
        },
        {
            path: '/:pathMatch(.*)*',
            name: 'main',
            component: () => import('@/layouts/MainLayout.vue'),
        },
    ],
});

// Navigation guard — enforce login before entering app routes.
router.beforeEach(async (to) => {
    const auth = useAuthStore();

    if (to.meta.public) {
        if (to.name === 'login' && auth.isAuthenticated) {
            try {
                await auth.ensureFresh();
                await auth.loadProfile();
                return { path: '/' };
            } catch {
                await auth.logout();
                return true;
            }
        }
        return true;
    }

    if (!auth.isAuthenticated) {
        return { path: '/login', query: { redirect: to.fullPath } };
    }

    try {
        await auth.ensureFresh();
        await auth.loadProfile(true);

        if (auth.mustChangePassword && to.name !== 'change-password') {
            return { path: '/change-password', query: { redirect: to.fullPath } };
        }

        if (!auth.mustChangePassword && to.name === 'change-password') {
            return { path: '/' };
        }

        if (to.name === 'admin-users' && !auth.isAdmin) {
            return { path: '/' };
        }

        return true;
    } catch {
        await auth.logout();
        return { path: '/login', query: { redirect: to.fullPath } };
    }
});

export default router;
