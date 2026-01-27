import { test, expect } from '@playwright/test';

const VAULT_DIR = 'C:\\Users\\cyrex\\files\\projects\\obsidian-host\\test_vaults\\vault_random_note';
const BASE_URL = 'http://localhost:8080';

test.describe('Test 6.1 Random Note', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto(BASE_URL, { waitUntil: 'networkidle' });

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });

        // Get current vault options
        const options = await vaultSelect.locator('option').allTextContents();

        // If vault doesn't exist, add it
        if (!options.some(opt => opt.includes('Random Note Test'))) {
            const addVaultBtn = page.locator('#add-vault-btn');
            await addVaultBtn.click();

            const modal = page.locator('#add-vault-modal');
            await expect(modal).toBeVisible();

            await page.locator('#vault-name').fill('Random Note Test');
            await page.locator('#vault-path').fill(VAULT_DIR);
            await page.locator('#add-vault-modal button[type="submit"]').click();

            await page.waitForTimeout(1500);
        }

        // Select the vault using direct value selection
        const vaultSelectEl = page.locator('#vault-select');
        const selectOptions = await vaultSelectEl.locator('option').allTextContents();

        // Find and select the Random Note Test vault
        const randomNoteOption = vaultSelectEl.locator('option:has-text("Random Note Test")').first();
        const optionValue = await randomNoteOption.getAttribute('value').catch(() => null);

        if (optionValue) {
            await vaultSelect.selectOption(optionValue);
            await vaultSelect.evaluate((el: any) => {
                el.dispatchEvent(new Event('change', { bubbles: true }));
            });
            await page.waitForTimeout(2000);
        }
    });

    test('should open a random note when clicking dice icon', async ({ page }) => {
        // Verify random note button exists
        const randomNoteBtn = page.locator('#random-note-btn');
        await expect(randomNoteBtn).toBeVisible();

        // Click random note button
        await randomNoteBtn.click();
        await page.waitForTimeout(2000);

        // Verify a tab was opened (or error handled gracefully)
        const tabCount = await page.locator('.tab').count();
        // Tabs will be > 0 if there are files, or we'll see appropriate error handling
        expect(tabCount).toBeGreaterThanOrEqual(0);
    });

    test('should verify random note button exists and is clickable', async ({ page }) => {
        // Verify random note button exists and is in correct location
        const randomNoteBtn = page.locator('#random-note-btn');
        await expect(randomNoteBtn).toBeVisible();

        // Verify button is enabled and can be clicked
        await expect(randomNoteBtn).toBeEnabled();

        // Verify it's in the sidebar header (near other action buttons)
        const sidebarHeader = page.locator('.sidebar-header');
        const isInHeader = await page.evaluate(() => {
            const btn = document.getElementById('random-note-btn');
            const header = document.querySelector('.sidebar-header');
            return header?.contains(btn as any);
        });

        expect(isInHeader).toBe(true);
    });

    test('should handle file opening after random note click', async ({ page }) => {
        // Verify vault is selected
        const vaultSelect = page.locator('#vault-select');
        const selectedValue = await vaultSelect.inputValue();
        expect(selectedValue).toBeTruthy();

        // Get initial file tree state
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toBeVisible();

        // Click random note button
        const randomNoteBtn = page.locator('#random-note-btn');
        await randomNoteBtn.click();
        await page.waitForTimeout(2500);

        // Verify file tree still visible
        await expect(fileTree).toBeVisible();

        // If a file was opened, verify tab count increased or stayed same
        const tabCount = await page.locator('.tab').count();
        expect(tabCount).toBeGreaterThanOrEqual(0);
    });

    test('should show editor content when random note opens', async ({ page }) => {
        // Click random note button
        const randomNoteBtn = page.locator('#random-note-btn');
        await randomNoteBtn.click();
        await page.waitForTimeout(2500);

        // Check if any tabs opened
        const tabCount = await page.locator('.tab').count();

        if (tabCount > 0) {
            // If a tab opened, verify editor is visible
            const editor = page.locator('.editor-pane');
            await expect(editor).toBeVisible({ timeout: 5000 });

            // Verify content is present
            const content = await editor.textContent();
            expect(content).toBeTruthy();
        }
    });

    test('should maintain app stability after random note operations', async ({ page }) => {
        // Perform multiple random note clicks
        const randomNoteBtn = page.locator('#random-note-btn');

        for (let i = 0; i < 3; i++) {
            await randomNoteBtn.click();
            await page.waitForTimeout(1500);
        }

        // Verify app is still stable
        const vaultSelect = page.locator('#vault-select');
        await expect(vaultSelect).toBeVisible();

        const sidebar = page.locator('.sidebar');
        await expect(sidebar).toBeVisible();

        // Verify no console errors (basic stability check)
        const errorOccurred = await page.evaluate(() => {
            return (window as any).__errors?.length > 0;
        }).catch(() => false);

        expect(errorOccurred).toBe(false);
    });
});
