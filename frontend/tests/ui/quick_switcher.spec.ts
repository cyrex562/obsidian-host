import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('Quick switcher', () => {
    test('opens with Ctrl+P and opens selected file', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: {
                [defaultVault.id]: [
                    { name: 'todo-list.md', path: 'todo-list.md', is_directory: false, modified: new Date().toISOString() },
                    { name: 'daily-note.md', path: 'daily-note.md', is_directory: false, modified: new Date().toISOString() },
                ],
            },
        });

        await page.goto('/');
        await expect(page.locator('button[title="New note"]')).toBeVisible();
        const input = page.locator('.v-dialog:visible input[placeholder^="Search files"]');
        for (let i = 0; i < 5; i++) {
            await page.evaluate(() => {
                document.dispatchEvent(new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, bubbles: true }));
                window.dispatchEvent(new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, bubbles: true }));
            });
            if (await input.count()) break;
            await page.waitForTimeout(150);
        }

        await expect(input).toBeVisible();
        await input.fill('todo');
        await page.keyboard.press('Enter');

        await expect(page.locator('.tab-item')).toContainText('todo-list.md');
        await expect(input).not.toBeVisible();
    });
});
