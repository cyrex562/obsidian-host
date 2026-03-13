import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('Theme mode', () => {
    test('saves updated theme preference', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        let lastTheme = 'dark';
        await installCommonAppMocks(page, { profile: defaultProfile, vaults: [defaultVault] });
        await page.route('**/api/preferences', async (route) => {
            const method = route.request().method();
            if (method === 'GET') {
                await route.fulfill({
                    status: 200,
                    contentType: 'application/json',
                    body: JSON.stringify({ theme: lastTheme, editor_mode: 'side_by_side', font_size: 14, window_layout: null }),
                });
                return;
            }
            if (method === 'PUT') {
                const payload = route.request().postDataJSON() as { theme: string };
                lastTheme = payload.theme;
                await route.fulfill({
                    status: 200,
                    contentType: 'application/json',
                    body: JSON.stringify({ theme: lastTheme, editor_mode: 'side_by_side', font_size: 14, window_layout: null }),
                });
                return;
            }
            await route.continue();
        });

        await page.goto('/');
        await page.locator('button[title="Theme"]').click();

        await expect.poll(() => lastTheme).toBe('light');
        await page.locator('button[title="Theme"]').click();
        await expect.poll(() => lastTheme).toBe('dark');
    });
});
