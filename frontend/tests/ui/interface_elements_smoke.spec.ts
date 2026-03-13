import { expect, test } from '@playwright/test';
import {
    defaultProfile,
    defaultVault,
    installCommonAppMocks,
    seedActiveVault,
    seedAuthTokens,
} from './helpers/appMocks';

test.describe('Core interface elements smoke coverage', () => {
    test('renders top bar, sidebar actions, search and plugin interfaces', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        await installCommonAppMocks(page, {
            profile: { ...defaultProfile, username: 'ui-admin', is_admin: true },
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [
                    {
                        name: 'README.md',
                        path: 'README.md',
                        is_directory: false,
                        modified: new Date().toISOString(),
                    },
                ],
            },
            plugins: [
                { id: 'word-count', name: 'Word Count', description: 'Counts words', enabled: true },
            ],
            searchResults: [
                {
                    path: 'README.md',
                    title: 'README',
                    matches: [{ line_number: 1, line_text: '# README', match_start: 2, match_end: 8 }],
                    score: 1,
                },
            ],
        });

        await page.goto('/');
        await expect(page.getByRole('banner').getByText(defaultVault.name)).toBeVisible();

        await expect(page.locator('button[title="Search (Ctrl+Shift+F)"]')).toBeVisible();
        await expect(page.locator('button[title="Plugins"]')).toBeVisible();
        await expect(page.locator('button[title="Theme"]')).toBeVisible();

        await expect(page.locator('button[title="New note"]')).toBeVisible();
        await expect(page.locator('button[title="New folder"]')).toBeVisible();
        await expect(page.locator('button[title="Refresh file tree"]')).toBeVisible();
        await expect(page.getByText('README.md')).toBeVisible();

        await page.locator('button[title="Search (Ctrl+Shift+F)"]').click();
        await expect(page.locator('.v-card-title', { hasText: 'Search' }).first()).toBeVisible();
        await page.getByRole('textbox', { name: 'Search', exact: true }).fill('README');
        await page.keyboard.press('Enter');
        await expect(page.getByText('README.md')).toBeVisible();
        await page.keyboard.press('Escape');

        await page.locator('button[title="Plugins"]').click();
        await expect(page.getByText('Plugins')).toBeVisible();
        await expect(page.getByText('Word Count')).toBeVisible();
        await page.getByRole('button', { name: 'Close' }).click();

        await page.getByRole('button', { name: 'ui-admin' }).click();
        await expect(page.getByText('Change password')).toBeVisible();
        await expect(page.getByText('Manage users')).toBeVisible();

        await page.getByText('Manage users').click();
        await expect(page).toHaveURL('/admin/users');
    });
});
