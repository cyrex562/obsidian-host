import type { Page } from '@playwright/test';

type UserProfile = {
    id: string;
    username: string;
    is_admin: boolean;
    must_change_password: boolean;
    groups: Array<{ id: string; name: string; created_at: string }>;
    auth_method: string;
};

type MockOptions = {
    profile?: UserProfile;
    vaults?: Array<{ id: string; name: string; path: string; path_exists: boolean; created_at: string; updated_at: string }>;
    treeByVaultId?: Record<string, unknown[]>;
    fileContentsByVaultId?: Record<string, Record<string, string>>;
    fileFrontmatterByVaultId?: Record<string, Record<string, Record<string, unknown>>>;
    plugins?: Array<{ id: string; name: string; description: string; enabled: boolean }>;
    searchResults?: Array<{ path: string; title: string; matches: Array<{ line_number: number; line_text: string; match_start: number; match_end: number }>; score: number }>;
};

export const defaultProfile: UserProfile = {
    id: 'u1',
    username: 'alice',
    is_admin: false,
    must_change_password: false,
    groups: [],
    auth_method: 'password',
};

export const defaultVault = {
    id: 'v1',
    name: 'Demo Vault',
    path: 'C:/vaults/demo',
    path_exists: true,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
};

export async function seedAuthTokens(page: Page, access = 'access-token', refresh = 'refresh-token') {
    await page.addInitScript(
        ({ accessToken, refreshToken }) => {
            localStorage.setItem('obsidian_access_token', accessToken);
            localStorage.setItem('obsidian_refresh_token', refreshToken);
            localStorage.setItem('obsidian_token_expires_at', String(Date.now() + 60 * 60 * 1000));
        },
        { accessToken: access, refreshToken: refresh },
    );
}

export async function seedActiveVault(page: Page, vaultId: string) {
    await page.addInitScript((id) => {
        localStorage.setItem('obsidian_active_vault', id);
    }, vaultId);
}

export async function installCommonAppMocks(page: Page, options: MockOptions = {}) {
    const profile = options.profile ?? defaultProfile;
    const vaults = [...(options.vaults ?? [])];
    const treeByVaultId = options.treeByVaultId ?? {};
    const fileContentsByVaultId = options.fileContentsByVaultId ?? {};
    const fileFrontmatterByVaultId = options.fileFrontmatterByVaultId ?? {};
    const plugins = options.plugins ?? [];
    const searchResults = options.searchResults ?? [];

    const prefs = {
        theme: 'dark',
        editor_mode: 'side_by_side',
        font_size: 14,
        window_layout: null,
    };

    await page.route('**/api/auth/refresh', async (route) => {
        await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ access_token: 'access-token', refresh_token: 'refresh-token', expires_in: 3600 }),
        });
    });

    await page.route('**/api/auth/me', async (route) => {
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(profile) });
    });

    await page.route('**/api/vaults', async (route) => {
        const method = route.request().method();
        if (method === 'GET') {
            await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(vaults) });
            return;
        }

        if (method === 'POST') {
            const payload = route.request().postDataJSON() as { name: string; path?: string };
            const slug = payload.name.trim().toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/(^-|-$)/g, '');
            const created = {
                id: `v-${vaults.length + 1}`,
                name: payload.name,
                path: payload.path ?? `C:/vaults/${slug || `vault-${vaults.length + 1}`}`,
                path_exists: true,
                created_at: new Date().toISOString(),
                updated_at: new Date().toISOString(),
            };
            vaults.push(created);
            treeByVaultId[created.id] = [];
            fileContentsByVaultId[created.id] = {};
            fileFrontmatterByVaultId[created.id] = {};
            await route.fulfill({ status: 201, contentType: 'application/json', body: JSON.stringify(created) });
            return;
        }

        await route.continue();
    });

    await page.route(/.*\/api\/vaults\/[^/]+$/, async (route) => {
        if (route.request().method() !== 'DELETE') {
            await route.continue();
            return;
        }

        const match = route.request().url().match(/\/api\/vaults\/([^/]+)$/);
        const vaultId = match?.[1] ?? '';
        const idx = vaults.findIndex((v) => v.id === vaultId);
        if (idx >= 0) vaults.splice(idx, 1);
        delete treeByVaultId[vaultId];
        delete fileContentsByVaultId[vaultId];
        delete fileFrontmatterByVaultId[vaultId];
        await route.fulfill({ status: 204, body: '' });
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/files$/, async (route) => {
        const method = route.request().method();
        const match = route.request().url().match(/\/api\/vaults\/([^/]+)\/files$/);
        const vaultId = match?.[1] ?? '';

        if (method === 'GET') {
            await route.fulfill({
                status: 200,
                contentType: 'application/json',
                body: JSON.stringify(treeByVaultId[vaultId] ?? []),
            });
            return;
        }

        if (method === 'POST') {
            const payload = route.request().postDataJSON() as { path: string; content?: string };
            const existingTree = (treeByVaultId[vaultId] as Array<any> | undefined) ?? [];
            existingTree.push({
                name: payload.path.split('/').pop() ?? payload.path,
                path: payload.path,
                is_directory: false,
                modified: new Date().toISOString(),
            });
            treeByVaultId[vaultId] = existingTree;

            const contentMap = (fileContentsByVaultId[vaultId] ??= {});
            contentMap[payload.path] = payload.content ?? '';
            const fmMap = (fileFrontmatterByVaultId[vaultId] ??= {});
            fmMap[payload.path] = {};

            await route.fulfill({
                status: 201,
                contentType: 'application/json',
                body: JSON.stringify({
                    path: payload.path,
                    content: contentMap[payload.path],
                    modified: new Date().toISOString(),
                    frontmatter: fmMap[payload.path],
                }),
            });
            return;
        }

        await route.continue();
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/files\/.+/, async (route) => {
        const method = route.request().method();
        const match = route.request().url().match(/\/api\/vaults\/([^/]+)\/files\/(.+)$/);
        const vaultId = match?.[1] ?? '';
        const rawPath = match?.[2] ?? '';
        const filePath = decodeURIComponent(rawPath);

        const contentMap = (fileContentsByVaultId[vaultId] ??= {});
        const fmMap = (fileFrontmatterByVaultId[vaultId] ??= {});

        if (method === 'GET') {
            await route.fulfill({
                status: 200,
                contentType: 'application/json',
                body: JSON.stringify({
                    path: filePath,
                    content: contentMap[filePath] ?? `# ${filePath}`,
                    modified: new Date().toISOString(),
                    frontmatter: fmMap[filePath] ?? {},
                }),
            });
            return;
        }

        if (method === 'PUT') {
            const payload = route.request().postDataJSON() as { content: string };
            contentMap[filePath] = payload.content;
            await route.fulfill({
                status: 200,
                contentType: 'application/json',
                body: JSON.stringify({
                    path: filePath,
                    content: payload.content,
                    modified: new Date().toISOString(),
                    frontmatter: fmMap[filePath] ?? {},
                }),
            });
            return;
        }

        if (method === 'DELETE') {
            delete contentMap[filePath];
            delete fmMap[filePath];
            const existingTree = (treeByVaultId[vaultId] as Array<any> | undefined) ?? [];
            treeByVaultId[vaultId] = existingTree.filter((n) => n.path !== filePath);
            await route.fulfill({ status: 204, body: '' });
            return;
        }

        await route.continue();
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/directories$/, async (route) => {
        if (route.request().method() !== 'POST') {
            await route.continue();
            return;
        }

        await route.fulfill({ status: 204, body: '' });
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/rename$/, async (route) => {
        if (route.request().method() !== 'POST') {
            await route.continue();
            return;
        }

        const match = route.request().url().match(/\/api\/vaults\/([^/]+)\/rename$/);
        const vaultId = match?.[1] ?? '';
        const payload = route.request().postDataJSON() as { from: string; to: string };
        const contentMap = (fileContentsByVaultId[vaultId] ??= {});
        const fmMap = (fileFrontmatterByVaultId[vaultId] ??= {});
        const existingTree = (treeByVaultId[vaultId] as Array<any> | undefined) ?? [];

        if (payload.from in contentMap) {
            contentMap[payload.to] = contentMap[payload.from];
            delete contentMap[payload.from];
        }

        if (payload.from in fmMap) {
            fmMap[payload.to] = fmMap[payload.from];
            delete fmMap[payload.from];
        }

        treeByVaultId[vaultId] = existingTree.map((n) => {
            if (n.path !== payload.from) return n;
            return {
                ...n,
                name: payload.to.split('/').pop() ?? payload.to,
                path: payload.to,
                modified: new Date().toISOString(),
            };
        });

        await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ new_path: payload.to }),
        });
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/render$/, async (route) => {
        const payload = route.request().postDataJSON() as { content: string };
        const content = payload.content ?? '';
        const withImageEmbeds = content.replace(/!\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g, (_, path: string, alt?: string) => {
            const safePath = path.trim();
            const safeAlt = (alt ?? path).trim();
            return `<img class="wiki-embed" data-original-link="${safePath}" alt="${safeAlt}" src="/api/raw/${safePath}" />`;
        });
        const withWikiLinks = withImageEmbeds.replace(/\[\[([^\]]+)\]\]/g, (_, target: string) => {
            const safeTarget = target.trim();
            return `<a class="wiki-link" data-original-link="${safeTarget}" href="${safeTarget}">${safeTarget}</a>`;
        });

        await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify(`<p>${withWikiLinks.replace(/\n/g, '<br/>')}</p>`),
        });
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/resolve-link$/, async (route) => {
        const payload = route.request().postDataJSON() as { link: string };
        await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({
                path: `${payload.link.replace(/\.md$/i, '')}.md`,
                exists: true,
                ambiguous: false,
                alternatives: [],
            }),
        });
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/reindex$/, async (route) => {
        await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ indexed_files: 1 }),
        });
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/random$/, async (route) => {
        await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ path: 'Random.md' }),
        });
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/daily$/, async (route) => {
        await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({
                path: 'Daily/2026-03-13.md',
                content: '# Daily',
                modified: new Date().toISOString(),
                frontmatter: {},
            }),
        });
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/recent$/, async (route) => {
        const method = route.request().method();
        if (method === 'GET') {
            await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify([]) });
            return;
        }
        if (method === 'POST') {
            await route.fulfill({ status: 204, body: '' });
            return;
        }
        await route.continue();
    });

    await page.route('**/api/plugins', async (route) => {
        if (route.request().method() === 'GET') {
            await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ plugins }) });
            return;
        }
        await route.continue();
    });

    await page.route(/.*\/api\/plugins\/[^/]+\/toggle$/, async (route) => {
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ success: true }) });
    });

    await page.route(/.*\/api\/vaults\/[^/]+\/search\?.*/, async (route) => {
        await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({
                results: searchResults,
                total_count: searchResults.length,
                page: 1,
                page_size: 50,
            }),
        });
    });

    await page.route('**/api/preferences', async (route) => {
        const method = route.request().method();
        if (method === 'GET') {
            await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(prefs) });
            return;
        }
        if (method === 'PUT') {
            Object.assign(prefs, route.request().postDataJSON() as Record<string, unknown>);
            await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(prefs) });
            return;
        }
        await route.continue();
    });

    await page.route('**/api/preferences/reset', async (route) => {
        await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({
                theme: 'dark',
                editor_mode: 'raw',
                font_size: 14,
                window_layout: null,
            }),
        });
    });
}
