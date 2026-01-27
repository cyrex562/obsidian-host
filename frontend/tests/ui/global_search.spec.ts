import { test, expect } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

const VAULT_DIR = path.join(__dirname, '../../../vault_global_search');

test.describe('Test 4.2 Global Search', () => {
    test.beforeAll(async () => {
        // Create test vault if it doesn't exist
        if (!fs.existsSync(VAULT_DIR)) {
            fs.mkdirSync(VAULT_DIR, { recursive: true });
        }

        // Create test files with various content for searching
        fs.writeFileSync(path.join(VAULT_DIR, 'todo.md'), '# Todo\n\n- [ ] Buy groceries\n- [ ] Write report\n- [x] Complete todo list');
        fs.writeFileSync(path.join(VAULT_DIR, 'project-notes.md'), '# Project Notes\n\nThe todo items for the project are listed elsewhere.');
        fs.writeFileSync(path.join(VAULT_DIR, 'meeting-notes.md'), '# Meeting Notes\n\nDiscussed the quarterly goals and objectives.');
        fs.writeFileSync(path.join(VAULT_DIR, 'ideas.md'), '# Random Ideas\n\nSome creative thoughts about future projects.');
    });

    test.afterAll(async () => {
        // Clean up test vault
        if (fs.existsSync(VAULT_DIR)) {
            fs.rmSync(VAULT_DIR, { recursive: true, force: true });
        }
    });

    test('should perform search when typing in search bar', async ({ page }) => {
        // Navigate to the app
        await page.goto('/');

        // Wait for vault selector
        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });

        // Check if vault already exists, if not add it
        const options = await vaultSelect.locator('option').allTextContents();
        if (!options.some(opt => opt.includes('Global Search Test'))) {
            const addVaultBtn = page.locator('#add-vault-btn');
            await addVaultBtn.click();

            const modal = page.locator('#add-vault-modal');
            await expect(modal).toBeVisible();

            await page.locator('#vault-name').fill('Global Search Test');
            await page.locator('#vault-path').fill(VAULT_DIR);
            await page.locator('#add-vault-modal button[type="submit"]').click();

            await page.waitForTimeout(500);
        }

        // Select the vault and manually trigger switchVault
        await vaultSelect.selectOption({ label: 'Global Search Test' });
        const selectedVaultId = await vaultSelect.inputValue();

        // Manually call switchVault via JavaScript
        await page.evaluate((vaultId) => {
            const app = (window as any).app;
            if (app && app.switchVault) {
                app.switchVault(vaultId);
            }
        }, selectedVaultId);
        await page.waitForTimeout(1000);

        // Reindex the vault to ensure search works
        await page.evaluate(async (vaultId) => {
            const response = await fetch(`/api/vaults/${vaultId}/reindex`, {
                method: 'POST'
            });
            if (!response.ok) {
                console.error('Reindex failed:', response.status);
            }
        }, selectedVaultId);
        await page.waitForTimeout(500);

        // Wait for file tree to load
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('todo.md', { timeout: 10000 });

        // Type in search bar
        const searchInput = page.locator('#search-input');
        await searchInput.click();
        await searchInput.type('todo', { delay: 100 });

        // Wait for debounced search + API response
        await page.waitForTimeout(800);

        // Verify search modal appears
        const searchModal = page.locator('#search-modal');
        await expect(searchModal).not.toHaveClass(/hidden/, { timeout: 5000 });

        // Verify search results are displayed
        const searchResults = page.locator('#search-results');
        await expect(searchResults).toBeVisible();
    });

    test('should display search results with matches', async ({ page }) => {
        // Navigate and set up vault
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Global Search Test' });
        await vaultSelect.dispatchEvent('change');
        await page.waitForTimeout(1000);

        // Wait for file tree
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('todo.md', { timeout: 10000 });

        // Search for "todo"
        const searchInput = page.locator('#search-input');
        await searchInput.fill('todo');
        await page.waitForTimeout(800);

        // Verify modal is open
        const searchModal = page.locator('#search-modal');
        await expect(searchModal).not.toHaveClass(/hidden/, { timeout: 5000 });

        // Verify at least one result item exists
        const resultItems = page.locator('.search-result-item');
        const count = await resultItems.count();
        expect(count).toBeGreaterThan(0);

        // Verify result items have expected structure
        const firstResult = resultItems.first();
        await expect(firstResult.locator('.search-result-title')).toBeVisible();
        await expect(firstResult.locator('.search-result-path')).toBeVisible();
    });

    test('should open file when clicking search result', async ({ page }) => {
        // Navigate and set up vault
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Global Search Test' });
        await vaultSelect.dispatchEvent('change');
        await page.waitForTimeout(1000);

        // Wait for file tree
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('todo.md', { timeout: 10000 });

        // Search for "todo"
        const searchInput = page.locator('#search-input');
        await searchInput.click();
        await searchInput.type('todo', { delay: 100 });
        await page.waitForTimeout(800);

        // Wait for modal to open
        const searchModal = page.locator('#search-modal');
        await expect(searchModal).not.toHaveClass(/hidden/, { timeout: 5000 });

        // Get first result and click it
        const resultItems = page.locator('.search-result-item');
        const firstResult = resultItems.first();

        // Get the file path from the result
        const filePath = await firstResult.locator('.search-result-path').textContent();

        // Click the result
        await firstResult.click();
        await page.waitForTimeout(500);

        // Verify modal is closed
        await expect(searchModal).toHaveClass(/hidden/);

        // Verify file opened in a tab
        if (filePath) {
            const fileName = filePath.split('/').pop();
            if (fileName) {
                const tabs = page.locator('.tab');
                await expect(tabs).toContainText(fileName);

                // Verify editor shows content
                const textarea = page.locator('textarea.editor-raw');
                const editorContent = await textarea.inputValue();
                expect(editorContent.length).toBeGreaterThan(0);
            }
        }
    });

    test('should show "No results found" for non-existent search', async ({ page }) => {
        // Navigate and set up vault
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Global Search Test' });
        await vaultSelect.dispatchEvent('change');
        await page.waitForTimeout(1000);

        // Wait for file tree
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('todo.md', { timeout: 10000 });

        // Search for something that doesn't exist
        const searchInput = page.locator('#search-input');
        await searchInput.click();
        await searchInput.type('xyzzythisshouldfindnothing', { delay: 100 });
        await page.waitForTimeout(800);

        // Verify modal opens
        const searchModal = page.locator('#search-modal');
        await expect(searchModal).not.toHaveClass(/hidden/, { timeout: 5000 });

        // Verify "No results found" message
        const searchResults = page.locator('#search-results');
        await expect(searchResults).toContainText('No results found');
    });

    test('should only search when query is at least 2 characters', async ({ page }) => {
        // Navigate and set up vault
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Global Search Test' });
        await vaultSelect.dispatchEvent('change');
        await page.waitForTimeout(1000);

        // Wait for file tree
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('todo.md', { timeout: 10000 });

        // Type single character
        const searchInput = page.locator('#search-input');
        await searchInput.click();
        await searchInput.type('t', { delay: 100 });
        await page.waitForTimeout(800);

        // Verify modal does not open (query too short)
        const searchModal = page.locator('#search-modal');
        await expect(searchModal).toHaveClass(/hidden/);

        // Type two characters
        await searchInput.clear();
        await searchInput.click();
        await searchInput.type('to', { delay: 100 });
        await page.waitForTimeout(800);

        // Verify modal now opens
        await expect(searchModal).not.toHaveClass(/hidden/, { timeout: 5000 });
    });
});
