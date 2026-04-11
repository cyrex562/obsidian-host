import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

test.describe('Plugin manager', () => {
    test('lists installed plugins and allows toggling', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        let toggleRequests = 0;
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            plugins: [
                { id: 'word-count', name: 'Word Count', description: 'Counts words', enabled: true },
                { id: 'daily-notes', name: 'Daily Notes', description: 'Daily note helper', enabled: false },
            ],
        });

        await page.route('**/api/plugins', async (route) => {
            await route.fulfill({
                status: 200,
                contentType: 'application/json',
                body: JSON.stringify({
                    plugins: [
                        {
                            manifest: {
                                id: 'word-count',
                                name: 'Word Count',
                                version: '1.0.0',
                                description: 'Counts words',
                            },
                            enabled: true,
                            state: 'unloaded',
                            path: './plugins/word-count',
                            config: null,
                            last_error: null,
                        },
                        {
                            manifest: {
                                id: 'daily-notes',
                                name: 'Daily Notes',
                                version: '1.1.0',
                                description: 'Daily note helper',
                            },
                            enabled: false,
                            state: 'disabled',
                            path: './plugins/daily-notes',
                            config: null,
                            last_error: null,
                        },
                    ],
                }),
            });
        });

        await page.route(/.*\/api\/plugins\/[^/]+\/toggle$/, async (route) => {
            toggleRequests += 1;
            await route.fulfill({
                status: 200,
                contentType: 'application/json',
                body: JSON.stringify({ success: true }),
            });
        });

        await page.goto('/');
        await page.locator('button[title="Plugins"]').click();

        await expect(page.getByText('Word Count')).toBeVisible();
        await expect(page.getByText('Daily Notes')).toBeVisible();

        await page.locator('.v-switch').first().click();
        await expect.poll(() => toggleRequests).toBeGreaterThan(0);
    });

    test('shows worldbuilding quick-create actions and opens the new entity dialog', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: { [defaultVault.id]: [] },
            plugins: [
                { id: 'com.codex.worldbuilding', name: 'Worldbuilding', description: 'Typed entities and relations', enabled: true },
            ],
            entityTypes: [
                {
                    id: 'character',
                    name: 'Character',
                    plugin_id: 'com.codex.worldbuilding',
                    color: '#4A90D9',
                    icon: 'mdi-account',
                    labels: ['graphable', 'person'],
                    show_on_create: ['full_name'],
                    display_field: 'full_name',
                    fields: [
                        { key: 'full_name', label: 'Full Name', field_type: 'string', required: true, values: [] },
                    ],
                },
                {
                    id: 'location',
                    name: 'Location',
                    plugin_id: 'com.codex.worldbuilding',
                    color: '#52B26B',
                    icon: 'mdi-map-marker',
                    labels: ['graphable', 'place'],
                    show_on_create: ['full_name'],
                    display_field: 'full_name',
                    fields: [
                        { key: 'full_name', label: 'Full Name', field_type: 'string', required: true, values: [] },
                    ],
                },
            ],
        });

        await page.goto('/');
        await page.locator('button[title="Plugins"]').click();

        await expect(page.getByRole('button', { name: 'New Character' })).toBeVisible();
        await expect(page.getByRole('button', { name: 'New Location' })).toBeVisible();

        await page.getByRole('button', { name: 'New Character' }).click();

        const dialog = page.locator('.v-dialog:visible').first();
        await expect(dialog.getByText('New Entity')).toBeVisible();
        await expect(dialog.getByLabel('Full Name').first()).toBeVisible();
    });
});
