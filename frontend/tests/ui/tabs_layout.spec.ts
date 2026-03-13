import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('Tabs and pane layout', () => {
    test('opens multiple tabs and switches active tab', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [
                    { name: 'first-note.md', path: 'first-note.md', is_directory: false, modified: new Date().toISOString() },
                    { name: 'second-note.md', path: 'second-note.md', is_directory: false, modified: new Date().toISOString() },
                ],
            },
            fileContentsByVaultId: {
                [defaultVault.id]: {
                    'first-note.md': '# First note',
                    'second-note.md': '# Second note',
                },
            },
        });

        await page.goto('/');
        await page.getByText('first-note.md').click();
        await page.getByText('second-note.md').click();

        await expect(page.locator('.tab-item')).toHaveCount(2);
        await page.locator('.tab-item', { hasText: 'first-note.md' }).click();
        await expect(page.locator('.tab-item.tab-active')).toContainText('first-note.md');
    });

    test('splits and closes panes via tab bar actions', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [{ name: 'layout.md', path: 'layout.md', is_directory: false, modified: new Date().toISOString() }],
            },
        });

        await page.goto('/');
        await page.getByText('layout.md').click();

        await page.locator('button[title="Split pane"]').first().click();
        await expect(page.locator('.pane-wrapper')).toHaveCount(2);

        await page.locator('button[title="Close pane"]').first().click();
        await expect(page.locator('.pane-wrapper')).toHaveCount(1);
    });
});
