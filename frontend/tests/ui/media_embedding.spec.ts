import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('Media embedding', () => {
    test('renders image wiki-embed in markdown preview', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [{ name: 'media-note.md', path: 'media-note.md', is_directory: false, modified: new Date().toISOString() }],
            },
            fileContentsByVaultId: {
                [defaultVault.id]: {
                    'media-note.md': '# Media\n\n![[test-image.png|My Image]]',
                },
            },
        });

        await page.goto('/');
        await page.getByText('media-note.md').click();

        await expect(page.locator('.markdown-preview img.wiki-embed')).toBeVisible();
        await expect(page.locator('.markdown-preview img.wiki-embed')).toHaveAttribute('alt', 'My Image');
    });
});
