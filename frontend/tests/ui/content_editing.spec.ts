import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('Content editing', () => {
    test('opens markdown content and autosaves updates', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        let writes = 0;
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [{ name: 'test-note.md', path: 'test-note.md', is_directory: false, modified: new Date().toISOString() }],
            },
            fileContentsByVaultId: {
                [defaultVault.id]: {
                    'test-note.md': '# Test Note\n\nInitial content here.',
                },
            },
        });

        await page.route(/.*\/api\/vaults\/[^/]+\/files\/.+/, async (route) => {
            if (route.request().method() === 'PUT') writes += 1;
            await route.fallback();
        });

        await page.goto('/');
        await page.getByText('test-note.md').click();
        await expect(page.locator('.markdown-editor')).toContainText('Initial content here.');

        await page.locator('.markdown-editor').click();
        await page.keyboard.type('\nUpdated line.');

        await expect(page.getByText('1 unsaved')).toBeVisible();
        await expect(page.getByText('Saved')).toBeVisible({ timeout: 5000 });
        await expect.poll(() => writes).toBeGreaterThan(0);
    });
});
