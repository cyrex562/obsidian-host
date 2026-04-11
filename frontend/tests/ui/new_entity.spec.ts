import { expect, test } from '@playwright/test';
import { defaultProfile, defaultVault, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';

const characterType = {
    id: 'character',
    name: 'Character',
    plugin_id: 'worldbuilding',
    color: '#4A90D9',
    icon: 'mdi-account',
    labels: ['graphable'],
    show_on_create: ['name'],
    display_field: 'name',
    fields: [
        { key: 'name', label: 'Name', field_type: 'string', required: true },
        { key: 'occupation', label: 'Occupation', field_type: 'string', required: false },
    ],
};

const locationTypeDef = {
    id: 'location',
    name: 'Location',
    plugin_id: 'worldbuilding',
    color: '#27AE60',
    icon: 'mdi-map-marker',
    labels: ['graphable'],
    show_on_create: [],
    display_field: 'name',
    fields: [
        { key: 'name', label: 'Name', field_type: 'string', required: true },
    ],
};

test.describe('New Entity dialog', () => {
    test('opens dialog when "New entity" button is clicked', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: { [defaultVault.id]: [] },
            entityTypes: [characterType],
        });

        await page.goto('/');

        await page.locator('button[title="New entity"]').click();

        const dialog = page.locator('.v-dialog:visible').first();
        await expect(dialog).toBeVisible();
        await expect(dialog.getByText('New Entity')).toBeVisible();
    });

    test('shows entity type selector with available types', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: { [defaultVault.id]: [] },
            entityTypes: [characterType, locationTypeDef],
        });

        await page.goto('/');
        await page.locator('button[title="New entity"]').click();

        const dialog = page.locator('.v-dialog:visible').first();
        await expect(dialog).toBeVisible();
        await expect(dialog.locator('.v-select', { hasText: 'Entity type' })).toBeVisible();
    });

    test('shows show_on_create fields after selecting entity type', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: { [defaultVault.id]: [] },
            entityTypes: [characterType],
        });

        await page.goto('/');
        await page.locator('button[title="New entity"]').click();

        const dialog = page.locator('.v-dialog:visible').first();
        await expect(dialog).toBeVisible();

        // With only one type, it should be auto-selected; Name is in show_on_create
        await expect(dialog.getByLabel('Name').first()).toBeVisible();
    });

    test('Create & Open button is disabled until file name is filled', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: { [defaultVault.id]: [] },
            entityTypes: [characterType],
        });

        await page.goto('/');
        await page.locator('button[title="New entity"]').click();

        const dialog = page.locator('.v-dialog:visible').first();
        const createBtn = dialog.getByRole('button', { name: /Create & Open/i });
        await expect(createBtn).toBeDisabled();

        // Fill file name field
        await dialog.getByLabel('File name').fill('aria.md');
        await expect(createBtn).toBeEnabled();
    });

    test('creates entity file and opens it in structural mode', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: { [defaultVault.id]: [] },
            fileContentsByVaultId: { [defaultVault.id]: {} },
            entityTypes: [characterType],
            entityTemplatesByTypeId: {
                character: `---\ncodex_type: character\ncodex_plugin: worldbuilding\nname: ""\n---\n`,
            },
        });

        await page.goto('/');
        await page.locator('button[title="New entity"]').click();

        const dialog = page.locator('.v-dialog:visible').first();
        await expect(dialog).toBeVisible();

        await dialog.getByLabel('File name').fill('Lyra');
        await dialog.getByRole('button', { name: /Create & Open/i }).click();

        // Dialog should close
        await expect(dialog).not.toBeVisible({ timeout: 8000 });

        // A tab for the new file should open
        await expect(page.locator('.tab-item')).toContainText(/Lyra/i);
    });

    test('Cancel button closes the dialog', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: { [defaultVault.id]: [] },
            entityTypes: [characterType],
        });

        await page.goto('/');
        await page.locator('button[title="New entity"]').click();

        const dialog = page.locator('.v-dialog:visible').first();
        await expect(dialog).toBeVisible();

        await dialog.getByRole('button', { name: 'Cancel' }).click();
        await expect(dialog).not.toBeVisible({ timeout: 3000 });
    });
});

test.describe('New Note dialog entity templates', () => {
    test('shows template selector when entity templates are available', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: { [defaultVault.id]: [] },
            entityTypes: [characterType, locationTypeDef],
        });

        await page.goto('/');
        await page.locator('button[title="New note"]').click();

        const dialog = page.locator('.v-dialog:visible').first();
        await expect(dialog.getByLabel('Template')).toBeVisible();
        await expect(dialog.locator('.v-select__selection-text')).toHaveText('Regular note');
    });

    test('routes entity template selection into the entity creation dialog', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, defaultVault.id);

        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [defaultVault],
            treeByVaultId: { [defaultVault.id]: [] },
            entityTypes: [characterType],
            fileContentsByVaultId: { [defaultVault.id]: {} },
            entityTemplatesByTypeId: {
                character: `---\ncodex_type: character\ncodex_plugin: worldbuilding\nname: ""\n---\n`,
            },
        });

        await page.goto('/');
        await page.locator('button[title="New note"]').click();

        const noteDialog = page.locator('.v-dialog:visible').first();
        await noteDialog.getByLabel('File name').fill('Lyra');
        await noteDialog.locator('.v-select').click();
        await page.locator('.v-list-item', { hasText: 'Character entity' }).first().click();
        await noteDialog.getByRole('button', { name: 'Continue' }).click();

        const entityDialog = page.locator('.v-dialog:visible').first();
        await expect(entityDialog.getByText('New Entity')).toBeVisible();
        await expect(entityDialog.getByLabel('File name')).toHaveValue('Lyra');
        await expect(entityDialog.getByLabel('Name').first()).toBeVisible();
    });
});
