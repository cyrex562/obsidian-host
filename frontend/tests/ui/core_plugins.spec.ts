import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('Core plugin entry points', () => {
    test('opens daily note from sidebar action', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);
        await installCommonAppMocks(page, { profile: defaultProfile, vaults: [defaultVault] });

        await page.goto('/');
        await page.locator('button[title="Open daily note"]').click();

        await expect(page.locator('.tab-item')).toContainText('2026-03-13.md');
    });

    test('shows plugin manager from top bar', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            plugins: [{ id: 'backlinks', name: 'Backlinks', description: 'Track links', enabled: true }],
        });

        await page.goto('/');
        await page.locator('button[title="Plugins"]').click();

        await expect(page.getByText('Plugins')).toBeVisible();
        await expect(page.getByText('Backlinks')).toBeVisible();
    });
});
