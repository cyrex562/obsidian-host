import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('File tree navigation', () => {
    test('renders nested structure and toggles folder expansion', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [
                    {
                        name: 'folder_b',
                        path: 'folder_b',
                        is_directory: true,
                        modified: new Date().toISOString(),
                        children: [
                            {
                                name: 'nested_note.md',
                                path: 'folder_b/nested_note.md',
                                is_directory: false,
                                modified: new Date().toISOString(),
                            },
                        ],
                    },
                    {
                        name: 'root_note.md',
                        path: 'root_note.md',
                        is_directory: false,
                        modified: new Date().toISOString(),
                    },
                ],
            },
            fileContentsByVaultId: {
                [defaultVault.id]: {
                    'folder_b/nested_note.md': '# Nested Note',
                },
            },
        });

        await page.goto('/');
        await expect(page.getByText('root_note.md')).toBeVisible();
        await expect(page.getByText('nested_note.md')).toBeVisible();

        const folderRow = page.locator('.file-tree-node', { hasText: 'folder_b' }).first();
        await folderRow.click();
        await expect(page.getByText('nested_note.md')).not.toBeVisible();

        await folderRow.click();
        await expect(page.getByText('nested_note.md')).toBeVisible();
    });
});
