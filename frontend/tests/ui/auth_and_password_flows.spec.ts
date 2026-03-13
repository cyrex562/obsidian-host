import { expect, test } from '@playwright/test';
import { defaultProfile, installCommonAppMocks, seedAuthTokens } from './helpers/appMocks';

test.describe('Auth and password UX flows', () => {
    test('redirects unauthenticated users to login', async ({ page }) => {
        await page.goto('/');
        await expect(page).toHaveURL(/\/login/);
        await expect(page.getByRole('button', { name: 'Sign In' })).toBeVisible();
    });

    test('logs in and lands in main app shell', async ({ page }) => {
        await page.route('**/api/auth/login', async (route) => {
            await route.fulfill({
                status: 200,
                contentType: 'application/json',
                body: JSON.stringify({ access_token: 'access-token', refresh_token: 'refresh-token', expires_in: 3600 }),
            });
        });

        await installCommonAppMocks(page, {
            profile: { ...defaultProfile, username: 'demo-user' },
            vaults: [],
        });

        await page.goto('/login');
        await page.getByLabel('Username').fill('demo-user');
        await page.getByLabel('Password').fill('correct-horse-battery-staple');
        await page.getByRole('button', { name: 'Sign In' }).click();

        await expect(page).toHaveURL('/');
        await expect(page.getByText('Select a vault to start.')).toBeVisible();
    });

    test('completes forced password-change flow and returns to app', async ({ page }) => {
        await seedAuthTokens(page);

        await installCommonAppMocks(page, {
            profile: { ...defaultProfile, username: 'rotating-user', must_change_password: true },
            vaults: [],
        });

        let mustChange = true;
        await page.unroute('**/api/auth/me');
        await page.route('**/api/auth/me', async (route) => {
            await route.fulfill({
                status: 200,
                contentType: 'application/json',
                body: JSON.stringify({ ...defaultProfile, username: 'rotating-user', must_change_password: mustChange }),
            });
        });

        await page.route('**/api/auth/change-password', async (route) => {
            mustChange = false;
            await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ success: true }) });
        });

        await page.goto('/change-password?redirect=/');
        await expect(page).toHaveURL(/\/change-password/);

        await page.getByRole('button', { name: 'Update password' }).click();
        await expect(page.getByText('Please fill in all password fields.')).toBeVisible();

        await page.getByLabel('Current password').fill('temp-password-123');
        await page.getByLabel('New password', { exact: true }).fill('short');
        await page.getByLabel('Confirm new password').fill('short');
        await page.getByRole('button', { name: 'Update password' }).click();
        await expect(page.getByText('New password must be at least 12 characters.')).toBeVisible();

        await page.getByLabel('New password', { exact: true }).fill('new-password-1234');
        await page.getByLabel('Confirm new password').fill('new-password-5678');
        await page.getByRole('button', { name: 'Update password' }).click();
        await expect(page.getByText('New password and confirmation do not match.')).toBeVisible();

        await page.getByLabel('Confirm new password').fill('new-password-1234');
        await page.getByRole('button', { name: 'Update password' }).click();

        await expect(page).toHaveURL('/');
        await expect(page.getByText('Select a vault to start.')).toBeVisible();
    });
});
