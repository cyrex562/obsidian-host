import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('Random note action', () => {
    test('opens a random note from sidebar action', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [{ name: 'Random.md', path: 'Random.md', is_directory: false, modified: new Date().toISOString() }],
            },
            fileContentsByVaultId: {
                [defaultVault.id]: {
                    'Random.md': '# Random note content',
                },
            },
        });

        await page.goto('/');
        await page.locator('button[title="Open random note"]').click();

        await expect(page.locator('.tab-item')).toContainText('Random.md');
        await expect(page.locator('.markdown-editor')).toContainText('Random note content');
    });
});
