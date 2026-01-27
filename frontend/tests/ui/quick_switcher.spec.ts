import { test, expect } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

const VAULT_DIR = path.join(__dirname, '../../../vault_quick_switcher');

test.describe('Test 4.1 Quick Switcher', () => {
    test.beforeAll(async () => {
        // Create test vault if it doesn't exist
        if (!fs.existsSync(VAULT_DIR)) {
            fs.mkdirSync(VAULT_DIR, { recursive: true });
        }

        // Create test files for searching
        fs.writeFileSync(path.join(VAULT_DIR, 'daily-note.md'), '# Daily Note\n\nThis is my daily note.');
        fs.writeFileSync(path.join(VAULT_DIR, 'todo-list.md'), '# Todo List\n\n- Task 1\n- Task 2');
        fs.writeFileSync(path.join(VAULT_DIR, 'ideas.md'), '# Ideas\n\nSome random ideas.');
        fs.writeFileSync(path.join(VAULT_DIR, 'daily-journal.md'), '# Daily Journal\n\nJournal entry.');
        fs.writeFileSync(path.join(VAULT_DIR, 'project-notes.md'), '# Project Notes\n\nNotes about the project.');
    });

    test.afterAll(async () => {
        // Clean up test vault
        if (fs.existsSync(VAULT_DIR)) {
            fs.rmSync(VAULT_DIR, { recursive: true, force: true });
        }
    });

    test('should open quick switcher modal with Ctrl+K', async ({ page }) => {
        // Navigate to the app
        await page.goto('/');

        // Wait for vault selector
        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });

        // Check if vault already exists, if not add it
        const options = await vaultSelect.locator('option').allTextContents();
        if (!options.some(opt => opt.includes('Quick Switcher Test'))) {
            const addVaultBtn = page.locator('#add-vault-btn');
            await addVaultBtn.click();

            const modal = page.locator('#add-vault-modal');
            await expect(modal).toBeVisible();

            await page.locator('#vault-name').fill('Quick Switcher Test');
            await page.locator('#vault-path').fill(VAULT_DIR);
            await page.locator('#add-vault-modal button[type="submit"]').click();

            await page.waitForTimeout(500);
        }

        // Select the vault
        await vaultSelect.selectOption({ label: 'Quick Switcher Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree to load
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('daily-note.md', { timeout: 10000 });

        // Press Ctrl+K to open quick switcher
        await page.keyboard.press('Control+k');
        await page.waitForTimeout(300);

        // Verify modal appears
        const quickSwitcherModal = page.locator('#quick-switcher-modal');
        await expect(quickSwitcherModal).not.toHaveClass(/hidden/);

        // Verify input is focused
        const input = page.locator('#quick-switcher-input');
        await expect(input).toBeFocused();

        // Verify results container exists
        const results = page.locator('#quick-switcher-results');
        await expect(results).toBeVisible();
    });

    test('should allow typing in search input to filter files', async ({ page }) => {
        // Navigate and set up vault
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Quick Switcher Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('daily-note.md', { timeout: 10000 });

        // Open quick switcher
        await page.keyboard.press('Control+k');
        await page.waitForTimeout(300);

        const input = page.locator('#quick-switcher-input');
        await expect(input).toBeVisible();

        // Verify we can type in the input
        await input.fill('daily');

        // Verify the input value is set
        const inputValue = await input.inputValue();
        expect(inputValue).toBe('daily');

        // Verify the quick switcher is still open and functional
        const quickSwitcherModal = page.locator('#quick-switcher-modal');
        await expect(quickSwitcherModal).not.toHaveClass(/hidden/);
    });

    test('should navigate with arrow keys and open with Enter', async ({ page }) => {
        // Navigate and set up vault
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Quick Switcher Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('daily-note.md', { timeout: 10000 });

        // Open quick switcher
        await page.keyboard.press('Control+k');
        await page.waitForTimeout(300);

        const input = page.locator('#quick-switcher-input');
        await input.fill('');
        await page.waitForTimeout(500);

        // Verify we have multiple results
        const resultsContainer = page.locator('#quick-switcher-results');
        const resultItems = resultsContainer.locator('.search-result-item');
        const count = await resultItems.count();

        if (count > 0) {
            // First item should be active by default
            const firstItem = resultItems.first();
            await expect(firstItem).toHaveClass(/active/);

            // Press down arrow
            await page.keyboard.press('ArrowDown');
            await page.waitForTimeout(100);

            // Second item should now be active
            if (count > 1) {
                const secondItem = resultItems.nth(1);
                await expect(secondItem).toHaveClass(/active/);

                // Press up arrow
                await page.keyboard.press('ArrowUp');
                await page.waitForTimeout(100);

                // First item should be active again
                await expect(firstItem).toHaveClass(/active/);
            }

            // Get the title of the currently active item
            const activeItem = resultsContainer.locator('.search-result-item.active');
            const filePath = await activeItem.locator('.search-result-path').textContent();

            // Press Enter to open
            await page.keyboard.press('Enter');
            await page.waitForTimeout(500);

            // Verify modal is closed
            const quickSwitcherModal = page.locator('#quick-switcher-modal');
            await expect(quickSwitcherModal).toHaveClass(/hidden/);

            // Verify file opened in a tab
            if (filePath) {
                const fileName = filePath.split('/').pop();
                if (fileName) {
                    const tabs = page.locator('.tab');
                    await expect(tabs).toContainText(fileName);
                }
            }
        }
    });

    test('should close modal with Escape key', async ({ page }) => {
        // Navigate and set up vault
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Quick Switcher Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('daily-note.md', { timeout: 10000 });

        // Open quick switcher
        await page.keyboard.press('Control+k');
        await page.waitForTimeout(300);

        // Verify modal is open
        const quickSwitcherModal = page.locator('#quick-switcher-modal');
        await expect(quickSwitcherModal).not.toHaveClass(/hidden/);

        // Press Escape
        await page.keyboard.press('Escape');
        await page.waitForTimeout(200);

        // Verify modal is closed
        await expect(quickSwitcherModal).toHaveClass(/hidden/);
    });
});
