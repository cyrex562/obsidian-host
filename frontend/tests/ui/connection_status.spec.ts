import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('Connection and save status chips', () => {
    test('shows websocket status chip and transitions unsaved/saved', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [{ name: 'status.md', path: 'status.md', is_directory: false, modified: new Date().toISOString() }],
            },
            fileContentsByVaultId: {
                [defaultVault.id]: {
                    'status.md': '# Status note',
                },
            },
        });

        await page.goto('/');
        await expect(page.getByText(/Connected|Offline/)).toBeVisible();

        await page.getByText('status.md').click();
        await page.locator('.markdown-editor').click();
        await page.keyboard.type('\nupdated');

        await expect(page.getByText('1 unsaved')).toBeVisible();
        await expect(page.getByText('Saved')).toBeVisible({ timeout: 5000 });
    });
});
