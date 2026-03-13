import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('Frontmatter metadata editor', () => {
    test('shows frontmatter panel and marks tab dirty after edit', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [{ name: 'project_alpha.md', path: 'project_alpha.md', is_directory: false, modified: new Date().toISOString() }],
            },
            fileContentsByVaultId: {
                [defaultVault.id]: {
                    'project_alpha.md': '# Project Alpha',
                },
            },
            fileFrontmatterByVaultId: {
                [defaultVault.id]: {
                    'project_alpha.md': {
                        status: 'wip',
                        priority: 'high',
                    },
                },
            },
        });

        await page.goto('/');
        await page.getByText('project_alpha.md').click();

        await expect(page.getByText('Frontmatter')).toBeVisible();
        await expect(page.getByText('status:')).toBeVisible();

        const firstFrontmatterInput = page.locator('.v-expansion-panel-text input').first();
        await firstFrontmatterInput.fill('done');
        await firstFrontmatterInput.blur();

        await expect(page.locator('.tab-item')).toContainText('●');
    });
});
