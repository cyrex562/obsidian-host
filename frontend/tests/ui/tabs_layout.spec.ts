import { test, expect } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

const VAULT_DIR = path.join(__dirname, '../../../vault_tabs_layout');

test.describe('Test 3.2 Tabs & Layout', () => {
    test.beforeAll(async () => {
        // Create test vault if it doesn't exist
        if (!fs.existsSync(VAULT_DIR)) {
            fs.mkdirSync(VAULT_DIR, { recursive: true });
        }

        // Create test files
        fs.writeFileSync(path.join(VAULT_DIR, 'first-note.md'), '# First Note\n\nThis is the first note.');
        fs.writeFileSync(path.join(VAULT_DIR, 'second-note.md'), '# Second Note\n\nThis is the second note.');
        fs.writeFileSync(path.join(VAULT_DIR, 'third-note.md'), '# Third Note\n\nThis is the third note.');
    });

    test.afterAll(async () => {
        // Clean up test vault
        if (fs.existsSync(VAULT_DIR)) {
            fs.rmSync(VAULT_DIR, { recursive: true, force: true });
        }
    });

    test('should open file in new tab with Ctrl+Click', async ({ page }) => {
        // Navigate to the app
        await page.goto('/');

        // Wait for vault selector
        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });

        // Check if vault already exists, if not add it
        const options = await vaultSelect.locator('option').allTextContents();
        if (!options.some(opt => opt.includes('Tabs Layout Test'))) {
            const addVaultBtn = page.locator('#add-vault-btn');
            await addVaultBtn.click();

            const modal = page.locator('#add-vault-modal');
            await expect(modal).toBeVisible();

            await page.locator('#vault-name').fill('Tabs Layout Test');
            await page.locator('#vault-path').fill(VAULT_DIR);
            await page.locator('#add-vault-modal button[type="submit"]').click();

            await page.waitForTimeout(500);
        }

        // Select the vault
        await vaultSelect.selectOption({ label: 'Tabs Layout Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree to load
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('first-note.md', { timeout: 10000 });

        // Open first file normally
        const firstFile = page.locator('.file-tree-item').filter({ hasText: 'first-note.md' }).first();
        await firstFile.click();
        await page.waitForTimeout(500);

        // Verify first tab is open
        const tabs = page.locator('.tab');
        await expect(tabs).toHaveCount(1);
        await expect(tabs.first()).toContainText('first-note.md');

        // Ctrl+Click on second file
        const secondFile = page.locator('.file-tree-item').filter({ hasText: 'second-note.md' }).first();
        await secondFile.click({ modifiers: ['Control'] });
        await page.waitForTimeout(500);

        // Verify two tabs are open
        await expect(tabs).toHaveCount(2);
        const tabNames = await tabs.allTextContents();
        expect(tabNames.some(name => name.includes('first-note.md'))).toBe(true);
        expect(tabNames.some(name => name.includes('second-note.md'))).toBe(true);
    });

    test('should switch tabs when clicking tab header', async ({ page }) => {
        // Navigate and set up vault
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Tabs Layout Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('first-note.md', { timeout: 10000 });

        // Open two files (first normally, second with Ctrl+Click)
        const firstFile = page.locator('.file-tree-item').filter({ hasText: 'first-note.md' }).first();
        await firstFile.click();
        await page.waitForTimeout(500);

        const secondFile = page.locator('.file-tree-item').filter({ hasText: 'second-note.md' }).first();
        await secondFile.click({ modifiers: ['Control'] });
        await page.waitForTimeout(500);

        // Verify second tab is active
        const secondTab = page.locator('.tab').filter({ hasText: 'second-note.md' }).first();
        await expect(secondTab).toHaveClass(/active/);

        // Verify editor shows second note content
        const textarea = page.locator('textarea.editor-raw');
        let editorContent = await textarea.inputValue();
        expect(editorContent).toContain('Second Note');

        // Click on first tab
        const firstTab = page.locator('.tab').filter({ hasText: 'first-note.md' }).first();
        const firstTabName = firstTab.locator('.tab-name');
        await firstTabName.click();
        await page.waitForTimeout(300);

        // Verify first tab is now active
        await expect(firstTab).toHaveClass(/active/);
        await expect(secondTab).not.toHaveClass(/active/);

        // Verify editor shows first note content
        editorContent = await textarea.inputValue();
        expect(editorContent).toContain('First Note');
    });

    test('should close tab when clicking close button', async ({ page }) => {
        // Navigate and set up vault
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Tabs Layout Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('first-note.md', { timeout: 10000 });

        // Open three files
        const firstFile = page.locator('.file-tree-item').filter({ hasText: 'first-note.md' }).first();
        await firstFile.click();
        await page.waitForTimeout(500);

        const secondFile = page.locator('.file-tree-item').filter({ hasText: 'second-note.md' }).first();
        await secondFile.click({ modifiers: ['Control'] });
        await page.waitForTimeout(500);

        const thirdFile = page.locator('.file-tree-item').filter({ hasText: 'third-note.md' }).first();
        await thirdFile.click({ modifiers: ['Control'] });
        await page.waitForTimeout(500);

        // Verify three tabs are open
        const tabs = page.locator('.tab');
        await expect(tabs).toHaveCount(3);

        // Find and click close button on second tab
        const secondTab = page.locator('.tab').filter({ hasText: 'second-note.md' }).first();
        const closeButton = secondTab.locator('.tab-close');
        await closeButton.click();
        await page.waitForTimeout(300);

        // Verify only two tabs remain
        await expect(tabs).toHaveCount(2);

        // Verify second tab is gone
        const tabNames = await tabs.allTextContents();
        expect(tabNames.some(name => name.includes('second-note.md'))).toBe(false);
        expect(tabNames.some(name => name.includes('first-note.md'))).toBe(true);
        expect(tabNames.some(name => name.includes('third-note.md'))).toBe(true);
    });
});
