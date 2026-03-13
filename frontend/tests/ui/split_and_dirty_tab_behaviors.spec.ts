import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('Additional pane and dirty-tab coverage', () => {
    test('opens file in split pane from context menu', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [
                    { name: 'split.md', path: 'split.md', is_directory: false, modified: new Date().toISOString() },
                ],
            },
        });

        await page.goto('/');
        const node = page.locator('.file-tree-node', { hasText: 'split.md' }).first();
        await node.click({ button: 'right' });
        await page.getByText('Open in split').last().click();

        await expect(page.locator('.pane-wrapper')).toHaveCount(2);
        await expect(page.locator('.tab-item')).toContainText('split.md');
    });

    test('respects close confirmation for dirty tabs', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [
                    { name: 'dirty.md', path: 'dirty.md', is_directory: false, modified: new Date().toISOString() },
                ],
            },
            fileContentsByVaultId: {
                [defaultVault.id]: {
                    'dirty.md': '# Dirty note',
                },
            },
        });

        await page.goto('/');
        await page.getByText('dirty.md').click();
        await page.locator('.markdown-editor').click();
        await page.keyboard.type('\nchange');

        page.once('dialog', async (dialog) => {
            await dialog.dismiss();
        });
        await page.locator('.tab-item .tab-close-btn').first().click();
        await expect(page.locator('.tab-item')).toContainText('dirty.md');

        page.once('dialog', async (dialog) => {
            await dialog.accept();
        });
        await page.locator('.tab-item .tab-close-btn').first().click();
        await expect(page.locator('.tab-item')).toHaveCount(0);
    });
});
