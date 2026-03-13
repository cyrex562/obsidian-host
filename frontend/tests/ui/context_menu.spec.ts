import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('File tree context menu actions', () => {
    test('renames and deletes file from context menu', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [{ name: 'note.md', path: 'note.md', is_directory: false, modified: new Date().toISOString() }],
            },
            fileContentsByVaultId: {
                [defaultVault.id]: {
                    'note.md': '# Note',
                },
            },
        });

        await page.goto('/');
        const noteNode = page.locator('.file-tree-node', { hasText: 'note.md' }).first();
        await noteNode.click({ button: 'right' });
        await page.getByText('Rename').last().click();

        const renameInput = page.locator('.file-tree-node input').first();
        await renameInput.fill('renamed.md');
        await renameInput.press('Enter');
        await expect(page.locator('.file-tree-node', { hasText: 'renamed.md' })).toBeVisible();

        page.once('dialog', async (dialog) => {
            await dialog.accept();
        });

        await page.locator('.file-tree-node', { hasText: 'renamed.md' }).first().click({ button: 'right' });
        await page.getByText('Delete').last().click();

        await expect(page.locator('.file-tree-node', { hasText: 'renamed.md' })).toHaveCount(0);
    });
});
