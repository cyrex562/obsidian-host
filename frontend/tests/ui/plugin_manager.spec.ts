import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('Plugin manager', () => {
    test('lists installed plugins and allows toggling', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        let toggleRequests = 0;
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            plugins: [
                { id: 'word-count', name: 'Word Count', description: 'Counts words', enabled: true },
                { id: 'daily-notes', name: 'Daily Notes', description: 'Daily note helper', enabled: false },
            ],
        });

        await page.route(/.*\/api\/plugins\/[^/]+\/toggle$/, async (route) => {
            toggleRequests += 1;
            await route.fulfill({
                status: 200,
                contentType: 'application/json',
                body: JSON.stringify({ success: true }),
            });
        });

        await page.goto('/');
        await page.locator('button[title="Plugins"]').click();

        await expect(page.getByText('Word Count')).toBeVisible();
        await expect(page.getByText('Daily Notes')).toBeVisible();

        await page.locator('.v-switch').first().click();
        await expect.poll(() => toggleRequests).toBeGreaterThan(0);
    });
});
