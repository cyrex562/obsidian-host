import { expect, test } from '@playwright/test';
import { defaultProfile, installCommonAppMocks, seedActiveVault, seedAuthTokens } from './helpers/appMocks';
import type { Page } from '@playwright/test';

const vaultA = {
    id: 'v-a',
    name: 'Vault Alpha',
    path: 'C:/vaults/alpha',
    path_exists: true,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
};

const vaultB = {
    id: 'v-b',
    name: 'Vault Beta',
    path: 'C:/vaults/beta',
    path_exists: true,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
};

async function installSharingMocks(page: Page) {
    const groups = [
        { id: 'g-editors', name: 'Editors', created_at: new Date().toISOString() },
    ];

    let shares = {
        owner_user_id: 'u1',
        user_shares: [
            {
                principal_type: 'user',
                principal_id: 'u2',
                principal_name: 'bob',
                role: 'viewer',
            },
        ],
        group_shares: [
            {
                principal_type: 'group',
                principal_id: 'g-editors',
                principal_name: 'Editors',
                role: 'editor',
            },
        ],
    };

    let members = [
        { user_id: 'u2', username: 'bob' },
    ];

    await page.route('**/api/groups', async (route) => {
        const method = route.request().method();
        if (method === 'GET') {
            await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(groups) });
            return;
        }

        if (method === 'POST') {
            const payload = route.request().postDataJSON() as { name: string };
            const created = { id: `g-${payload.name.toLowerCase()}`, name: payload.name, created_at: new Date().toISOString() };
            groups.push(created);
            await route.fulfill({ status: 201, contentType: 'application/json', body: JSON.stringify(created) });
            return;
        }

        await route.fallback();
    });

    await page.route('**/api/groups/*/members', async (route) => {
        const method = route.request().method();

        if (method === 'GET') {
            await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(members) });
            return;
        }

        if (method === 'POST') {
            const payload = route.request().postDataJSON() as { username?: string };
            const username = payload.username ?? 'user';
            const created = { user_id: `u-${username}`, username };
            members = [...members, created];
            await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(members) });
            return;
        }

        await route.fallback();
    });

    await page.route('**/api/groups/*/members/*', async (route) => {
        if (route.request().method() !== 'DELETE') {
            await route.fallback();
            return;
        }

        const target = route.request().url().split('/').pop() ?? '';
        members = members.filter((m) => m.user_id !== target);
        await route.fulfill({ status: 204, body: '' });
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/shares$/, async (route) => {
        if (route.request().method() !== 'GET') {
            await route.fallback();
            return;
        }
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(shares) });
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/shares\/users$/, async (route) => {
        if (route.request().method() !== 'POST') {
            await route.fallback();
            return;
        }

        const payload = route.request().postDataJSON() as { username?: string; role: string };
        shares = {
            ...shares,
            user_shares: [
                ...shares.user_shares,
                {
                    principal_type: 'user',
                    principal_id: `u-${payload.username ?? 'user'}`,
                    principal_name: payload.username ?? 'user',
                    role: payload.role,
                },
            ],
        };
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(shares) });
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/shares\/users\/[^/]+$/, async (route) => {
        if (route.request().method() !== 'DELETE') {
            await route.fallback();
            return;
        }

        const userId = route.request().url().split('/').pop() ?? '';
        shares = {
            ...shares,
            user_shares: shares.user_shares.filter((share) => share.principal_id !== userId),
        };
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(shares) });
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/shares\/groups$/, async (route) => {
        if (route.request().method() !== 'POST') {
            await route.fallback();
            return;
        }

        const payload = route.request().postDataJSON() as { group_id: string; role: string };
        const groupName = groups.find((g) => g.id === payload.group_id)?.name ?? payload.group_id;
        shares = {
            ...shares,
            group_shares: [
                ...shares.group_shares,
                {
                    principal_type: 'group',
                    principal_id: payload.group_id,
                    principal_name: groupName,
                    role: payload.role,
                },
            ],
        };
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(shares) });
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/shares\/groups\/[^/]+$/, async (route) => {
        if (route.request().method() !== 'DELETE') {
            await route.continue();
            return;
        }

        const groupId = route.request().url().split('/').pop() ?? '';
        shares = {
            ...shares,
            group_shares: shares.group_shares.filter((share) => share.principal_id !== groupId),
        };
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(shares) });
    });
}

test.describe('Vault management', () => {
    test('loads active vault from persisted selection', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, vaultA.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [vaultA, vaultB],
            treeByVaultId: {
                [vaultA.id]: [{ name: 'alpha.md', path: 'alpha.md', is_directory: false, modified: new Date().toISOString() }],
                [vaultB.id]: [{ name: 'beta.md', path: 'beta.md', is_directory: false, modified: new Date().toISOString() }],
            },
        });

        await page.goto('/');
        await expect(page.getByText('alpha.md')).toBeVisible();
        await expect(page.getByRole('banner').getByText('Vault Alpha')).toBeVisible();
    });

    test('loads alternate active vault id and tree', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, vaultB.id);
        await installCommonAppMocks(page, {
            profile: defaultProfile,
            vaults: [vaultA, vaultB],
            treeByVaultId: {
                [vaultA.id]: [{ name: 'alpha.md', path: 'alpha.md', is_directory: false, modified: new Date().toISOString() }],
                [vaultB.id]: [{ name: 'beta.md', path: 'beta.md', is_directory: false, modified: new Date().toISOString() }],
            },
        });

        await page.goto('/');
        await expect(page.getByText('beta.md')).toBeVisible();
        await expect(page.getByRole('banner').getByText('Vault Beta')).toBeVisible();
    });

    test('opens vault manager from sidebar controls', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, vaultA.id);
        await installCommonAppMocks(page, { profile: defaultProfile, vaults: [vaultA] });

        await page.goto('/');
        await page.locator('button:has(.mdi-cog)').first().click();
        await expect(page.getByText('Vault Manager')).toBeVisible();
        await expect(page.getByRole('button', { name: 'Add' })).toBeVisible();
    });

    test('manages sharing entries for users and groups', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, vaultA.id);
        await installCommonAppMocks(page, { profile: defaultProfile, vaults: [vaultA] });
        await installSharingMocks(page);

        await page.goto('/');
        await page.locator('button:has(.mdi-cog)').first().click();

        await expect(page.getByText('Current access')).toBeVisible();
        await expect(page.getByText('Group: Editors')).toBeVisible();

        await page.locator('button[title="Revoke group share"]').first().click();
        await expect(page.getByText('Group: Editors')).toHaveCount(0);
    });

    test('creates groups and manages group members', async ({ page }) => {
        await seedAuthTokens(page);
        await seedActiveVault(page, vaultA.id);
        await installCommonAppMocks(page, { profile: defaultProfile, vaults: [vaultA] });
        await installSharingMocks(page);

        await page.goto('/');
        await page.locator('button:has(.mdi-cog)').first().click();

        await page.getByLabel('Create group').fill('Reviewers');
        await page.getByRole('button', { name: 'Create' }).click();

        await expect(page.locator('.v-list-item-subtitle', { hasText: 'u2' })).toHaveCount(1);

        await page.getByLabel('Add member by username').fill('dana');
        await page.getByRole('button', { name: 'Add' }).first().click();
        await expect(page.getByText('dana', { exact: true })).toBeVisible();

        await page
            .locator('.v-list-item', { has: page.locator('.v-list-item-subtitle', { hasText: 'u2' }) })
            .locator('button')
            .first()
            .click();
        await expect(page.locator('.v-list-item-subtitle', { hasText: 'u2' })).toHaveCount(0);
    });
});
