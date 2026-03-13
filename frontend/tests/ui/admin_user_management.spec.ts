import { expect, test } from '@playwright/test';
import { defaultProfile, installCommonAppMocks, seedAuthTokens } from './helpers/appMocks';

test.describe('Admin user management', () => {
    test('blocks non-admin users from /admin/users', async ({ page }) => {
        await seedAuthTokens(page);
        await installCommonAppMocks(page, {
            profile: { ...defaultProfile, is_admin: false, must_change_password: false },
            vaults: [],
        });

        await page.goto('/admin/users');
        await expect(page).toHaveURL('/');
    });

    test('allows admins to list and create users', async ({ page }) => {
        await seedAuthTokens(page);

        const users = [
            {
                id: 'u-admin',
                username: 'admin',
                is_admin: true,
                must_change_password: false,
                created_at: new Date().toISOString(),
            },
        ];

        await installCommonAppMocks(page, {
            profile: { ...defaultProfile, username: 'admin', is_admin: true },
            vaults: [],
        });

        await page.route('**/api/admin/users', async (route) => {
            const method = route.request().method();
            if (method === 'GET') {
                await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(users) });
                return;
            }

            if (method === 'POST') {
                const payload = route.request().postDataJSON() as { username: string; is_admin?: boolean };
                const created = {
                    id: `u-${payload.username}`,
                    username: payload.username,
                    temporary_password: 'TempPassword-12345',
                    is_admin: !!payload.is_admin,
                    must_change_password: true,
                };

                users.push({
                    id: created.id,
                    username: created.username,
                    is_admin: created.is_admin,
                    must_change_password: created.must_change_password,
                    created_at: new Date().toISOString(),
                });

                await route.fulfill({ status: 201, contentType: 'application/json', body: JSON.stringify(created) });
                return;
            }

            await route.continue();
        });

        await page.goto('/admin/users');
        await expect(page.getByRole('heading', { name: 'Admin · User Management' })).toBeVisible();
        await expect(page.getByRole('cell', { name: 'admin', exact: true })).toBeVisible();

        await page.getByLabel('Username').fill('new-user');
        await page.getByLabel('Temporary password (optional)').fill('OneTimePassword-1234');
        await page.getByRole('button', { name: 'Create user' }).click();

        await expect(page.getByText('Temporary password for new-user')).toBeVisible();
        await expect(page.getByText('TempPassword-12345')).toBeVisible();
        await expect(page.getByRole('cell', { name: 'new-user', exact: true })).toBeVisible();
    });
});
