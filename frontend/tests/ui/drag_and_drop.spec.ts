import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('Tree stability around drag/drop interactions', () => {
    test('keeps file tree responsive when drag events are dispatched', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [
                    { name: 'note.md', path: 'note.md', is_directory: false, modified: new Date().toISOString() },
                    { name: 'Archive', path: 'Archive', is_directory: true, modified: new Date().toISOString(), children: [] },
                ],
            },
        });

        await page.goto('/');

        const source = page.locator('.file-tree-node', { hasText: 'note.md' }).first();
        const target = page.locator('.file-tree-node', { hasText: 'Archive' }).first();

        await source.dispatchEvent('dragstart');
        await target.dispatchEvent('dragover');
        await target.dispatchEvent('drop');

        await expect(page.locator('.file-tree-node', { hasText: 'note.md' })).toBeVisible();
        await source.click();
        await expect(page.locator('.tab-item')).toContainText('note.md');
    });
});
