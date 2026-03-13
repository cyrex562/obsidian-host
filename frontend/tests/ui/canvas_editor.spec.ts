import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('Canvas files', () => {
    test('shows canvas files in tree and opens them as non-editable content', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [
                    { name: 'Project_Flow.canvas', path: 'Project_Flow.canvas', is_directory: false, modified: new Date().toISOString() },
                ],
            },
            fileContentsByVaultId: {
                [defaultVault.id]: {
                    'Project_Flow.canvas': '{"nodes":[],"edges":[]}',
                },
            },
        });

        await page.goto('/');
        await expect(page.getByText('Project_Flow.canvas')).toBeVisible();

        await page.getByText('Project_Flow.canvas').click();
        await expect(page.locator('.tab-item')).toContainText('Project_Flow.canvas');
        await expect(page.getByText('Binary file — cannot be edited here.')).toBeVisible();
    });
});
