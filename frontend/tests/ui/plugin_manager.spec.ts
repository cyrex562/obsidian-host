import { test, expect } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

test.describe('Test 5.1 Plugin Manager UI', () => {
    const VAULT_DIR = path.join(process.cwd(), '..', 'test_vaults', 'vault_plugin_manager');

    test.beforeAll(async () => {
        // Create test vault if it doesn't exist
        if (!fs.existsSync(VAULT_DIR)) {
            fs.mkdirSync(VAULT_DIR, { recursive: true });
        }

        // Create a simple test file
        fs.writeFileSync(path.join(VAULT_DIR, 'test-note.md'), '# Test Note\n\nThis is a test note for plugin manager.');
    });

    test.afterAll(async () => {
        // Clean up test vault
        if (fs.existsSync(VAULT_DIR)) {
            fs.rmSync(VAULT_DIR, { recursive: true, force: true });
        }
    });

    test('should open Plugin Manager modal when clicking plugin icon', async ({ page }) => {
        // Navigate to app
        await page.goto('/');

        // Wait for app to load
        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });

        // Check if vault already exists, if not add it
        const options = await vaultSelect.locator('option').allTextContents();
        if (!options.some(opt => opt.includes('Plugin Manager Test'))) {
            const addVaultBtn = page.locator('#add-vault-btn');
            await addVaultBtn.click();

            const modal = page.locator('#add-vault-modal');
            await expect(modal).toBeVisible();

            await page.locator('#vault-name').fill('Plugin Manager Test');
            await page.locator('#vault-path').fill(VAULT_DIR);
            await page.locator('#add-vault-modal button[type="submit"]').click();

            await page.waitForTimeout(500);
        }

        // Select the vault
        await vaultSelect.selectOption({ label: 'Plugin Manager Test' });
        const selectedVaultId = await vaultSelect.inputValue();

        // Manually call switchVault
        await page.evaluate((vaultId) => {
            const app = (window as any).app;
            if (app && app.switchVault) {
                app.switchVault(vaultId);
            }
        }, selectedVaultId);
        await page.waitForTimeout(1000);

        // Click plugin manager button
        const pluginBtn = page.locator('#plugin-manager-btn');
        await expect(pluginBtn).toBeVisible();
        await pluginBtn.click();

        // Verify modal appears
        const modal = page.locator('#plugin-manager-modal');
        await expect(modal).not.toHaveClass(/hidden/);

        // Verify modal has title
        await expect(modal.locator('h2')).toContainText('Plugin Manager');

        // Verify tabs are visible
        await expect(modal.locator('.plugin-tab-btn[data-tab="installed"]')).toBeVisible();
        await expect(modal.locator('.plugin-tab-btn[data-tab="browse"]')).toBeVisible();
        await expect(modal.locator('.plugin-tab-btn[data-tab="settings"]')).toBeVisible();
    });

    test('should list installed core plugins', async ({ page }) => {
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Plugin Manager Test' });
        const selectedVaultId = await vaultSelect.inputValue();

        await page.evaluate((vaultId) => {
            const app = (window as any).app;
            if (app && app.switchVault) {
                app.switchVault(vaultId);
            }
        }, selectedVaultId);
        await page.waitForTimeout(1000);

        // Open plugin manager
        await page.locator('#plugin-manager-btn').click();

        // Wait for plugins to load (loadPlugins is called automatically)
        await page.waitForTimeout(1500);

        // Verify plugins list container exists
        const pluginsList = page.locator('#installed-plugins-list');
        await expect(pluginsList).toBeVisible();

        // Verify at least some plugins are listed
        const pluginItems = page.locator('.plugin-item');
        const count = await pluginItems.count();
        expect(count).toBeGreaterThan(0);

        // Verify we can find plugins
        const pluginNames = await page.locator('.plugin-item-name').allTextContents();

        // Should have some recognizable plugins
        expect(pluginNames.length).toBeGreaterThan(0);

        // Check for at least one plugin (Example Plugin, Daily Notes, Word Count, or Backlinks)
        const hasPlugin = pluginNames.some(name =>
            name.includes('Example Plugin') ||
            name.includes('Daily Notes') ||
            name.includes('Word Count') ||
            name.includes('Backlinks')
        );
        expect(hasPlugin).toBeTruthy();
    });

    test('should display plugin status badges', async ({ page }) => {
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Plugin Manager Test' });
        const selectedVaultId = await vaultSelect.inputValue();

        await page.evaluate((vaultId) => {
            const app = (window as any).app;
            if (app && app.switchVault) {
                app.switchVault(vaultId);
            }
        }, selectedVaultId);
        await page.waitForTimeout(1000);

        // Open plugin manager
        await page.locator('#plugin-manager-btn').click();

        // Wait for plugins to load (loadPlugins is called automatically)
        await page.waitForTimeout(1000);

        // Verify status badges exist
        const statusBadges = page.locator('.plugin-state-badge');
        const count = await statusBadges.count();
        expect(count).toBeGreaterThan(0);

        // Verify badges have appropriate classes
        const firstBadge = statusBadges.first();
        const badgeClasses = await firstBadge.getAttribute('class');
        expect(badgeClasses).toBeTruthy();

        // Should have plugin-state-badge class
        expect(badgeClasses).toContain('plugin-state-badge');

        // Should have a state-specific class (loaded, unloaded, disabled, or failed)
        const hasStateClass =
            badgeClasses?.includes('plugin-state-loaded') ||
            badgeClasses?.includes('plugin-state-unloaded') ||
            badgeClasses?.includes('plugin-state-disabled') ||
            badgeClasses?.includes('plugin-state-failed');
        expect(hasStateClass).toBeTruthy();

        // Verify badge text shows status
        const badgeTexts = await statusBadges.allTextContents();
        expect(badgeTexts.every(text => text.length > 0)).toBeTruthy();
    });

    test('should close plugin manager modal', async ({ page }) => {
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Plugin Manager Test' });
        const selectedVaultId = await vaultSelect.inputValue();

        await page.evaluate((vaultId) => {
            const app = (window as any).app;
            if (app && app.switchVault) {
                app.switchVault(vaultId);
            }
        }, selectedVaultId);
        await page.waitForTimeout(1000);

        // Open plugin manager
        await page.locator('#plugin-manager-btn').click();

        const modal = page.locator('#plugin-manager-modal');
        await expect(modal).not.toHaveClass(/hidden/);

        // Click close button
        const closeBtn = modal.locator('button[data-close-modal="plugin-manager-modal"]');
        await closeBtn.click();

        // Verify modal is closed
        await expect(modal).toHaveClass(/hidden/);
    });
});

test.describe('Test 5.2 Plugin Actions', () => {
    const VAULT_DIR = path.join(process.cwd(), '..', 'test_vaults', 'vault_plugin_actions');

    test.beforeAll(async () => {
        // Create test vault if it doesn't exist
        if (!fs.existsSync(VAULT_DIR)) {
            fs.mkdirSync(VAULT_DIR, { recursive: true });
        }

        // Create a simple test file
        fs.writeFileSync(path.join(VAULT_DIR, 'test-note.md'), '# Test Note\n\nThis is a test note for plugin actions.');
    });

    test.afterAll(async () => {
        // Clean up test vault
        if (fs.existsSync(VAULT_DIR)) {
            fs.rmSync(VAULT_DIR, { recursive: true, force: true });
        }
    });

    test('should toggle plugin enable/disable', async ({ page }) => {
        // Clear browser cache and reload
        await page.goto('/', { waitUntil: 'networkidle' });
        await page.evaluate(() => {
            if ('caches' in window) {
                caches.keys().then(names => {
                    names.forEach(name => caches.delete(name));
                });
            }
        });
        await page.reload({ waitUntil: 'networkidle' });

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });

        // Check if vault already exists, if not add it
        const options = await vaultSelect.locator('option').allTextContents();
        if (!options.some(opt => opt.includes('Plugin Actions Test'))) {
            const addVaultBtn = page.locator('#add-vault-btn');
            await addVaultBtn.click();

            const modal = page.locator('#add-vault-modal');
            await expect(modal).toBeVisible();

            await page.locator('#vault-name').fill('Plugin Actions Test');
            await page.locator('#vault-path').fill(VAULT_DIR);
            await page.locator('#add-vault-modal button[type="submit"]').click();

            await page.waitForTimeout(500);
        }

        await vaultSelect.selectOption({ label: 'Plugin Actions Test' });
        const selectedVaultId = await vaultSelect.inputValue();

        await page.evaluate((vaultId) => {
            const app = (window as any).app;
            if (app && app.switchVault) {
                app.switchVault(vaultId);
            }
        }, selectedVaultId);
        await page.waitForTimeout(1000);

        // Open plugin manager
        await page.locator('#plugin-manager-btn').click();
        await page.waitForTimeout(2000);

        // Debug: Check HTML content
        const pluginsListHTML = await page.locator('#installed-plugins-list').innerHTML();
        console.log('Plugins list HTML:', pluginsListHTML);

        // Find the first plugin with an enable/disable button
        const toggleBtn = page.locator('.plugin-toggle-btn').first();
        await expect(toggleBtn).toBeVisible();

        // Get initial button text
        const initialText = await toggleBtn.textContent();
        const wasEnabled = initialText?.includes('Disable');

        // Click toggle button
        await toggleBtn.click();
        await page.waitForTimeout(1000);

        // Verify button text changed
        const newText = await toggleBtn.textContent();
        if (wasEnabled) {
            expect(newText).toContain('Enable');
        } else {
            expect(newText).toContain('Disable');
        }

        // Toggle back
        await toggleBtn.click();
        await page.waitForTimeout(1000);

        // Verify it changed back
        const finalText = await toggleBtn.textContent();
        expect(finalText).toBe(initialText);
    });

    test('should show settings when clicking settings button', async ({ page }) => {
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Plugin Actions Test' });
        const selectedVaultId = await vaultSelect.inputValue();

        await page.evaluate((vaultId) => {
            const app = (window as any).app;
            if (app && app.switchVault) {
                app.switchVault(vaultId);
            }
        }, selectedVaultId);
        await page.waitForTimeout(1000);

        // Open plugin manager
        await page.locator('#plugin-manager-btn').click();
        await page.waitForTimeout(1500);

        // Click settings button on first plugin
        const settingsBtn = page.locator('.plugin-settings-btn').first();
        await expect(settingsBtn).toBeVisible();
        await settingsBtn.click();

        // Verify settings tab is shown
        await page.waitForTimeout(500);
        const settingsTab = page.locator('#plugin-tab-settings');
        await expect(settingsTab).toBeVisible();
        await expect(settingsTab).not.toHaveClass(/hidden/);

        // Verify settings content is shown
        await expect(settingsTab.locator('h3')).toBeVisible();
        await expect(settingsTab.locator('.plugin-settings-container')).toBeVisible();
    });

    test('should have enable/disable and settings buttons for each plugin', async ({ page }) => {
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Plugin Actions Test' });
        const selectedVaultId = await vaultSelect.inputValue();

        await page.evaluate((vaultId) => {
            const app = (window as any).app;
            if (app && app.switchVault) {
                app.switchVault(vaultId);
            }
        }, selectedVaultId);
        await page.waitForTimeout(1000);

        // Open plugin manager
        await page.locator('#plugin-manager-btn').click();
        await page.waitForTimeout(1500);

        // Verify plugins have toggle buttons
        const toggleBtns = page.locator('.plugin-toggle-btn');
        const toggleCount = await toggleBtns.count();
        expect(toggleCount).toBeGreaterThan(0);

        // Verify plugins have settings buttons
        const settingsBtns = page.locator('.plugin-settings-btn');
        const settingsCount = await settingsBtns.count();
        expect(settingsCount).toBeGreaterThan(0);

        // Verify counts match (each plugin should have both buttons)
        expect(toggleCount).toBe(settingsCount);
    });
});
