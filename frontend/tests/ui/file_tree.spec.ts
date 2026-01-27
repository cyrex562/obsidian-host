import { test, expect } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

const FIXTURES_DIR = path.resolve(__dirname, '../../tests/fixtures');
const VAULT_TREE_DIR = path.join(FIXTURES_DIR, 'vault_tree');
const EMPTY_VAULT_DIR = path.join(FIXTURES_DIR, 'vault_empty');

test.beforeAll(async () => {
    // Setup test vault with folder structure
    if (!fs.existsSync(FIXTURES_DIR)) fs.mkdirSync(FIXTURES_DIR, { recursive: true });

    if (fs.existsSync(VAULT_TREE_DIR)) fs.rmSync(VAULT_TREE_DIR, { recursive: true, force: true });
    fs.mkdirSync(VAULT_TREE_DIR);

    // Create structure:
    // - root_note.md
    // - folder_a/
    //   - note_a.md
    // - folder_b/
    //   - note_b.md
    //   - subfolder_b1/
    //     - nested_note.md

    fs.writeFileSync(path.join(VAULT_TREE_DIR, 'root_note.md'), '# Root Note');

    fs.mkdirSync(path.join(VAULT_TREE_DIR, 'folder_a'));
    fs.writeFileSync(path.join(VAULT_TREE_DIR, 'folder_a', 'note_a.md'), '# Note A');

    fs.mkdirSync(path.join(VAULT_TREE_DIR, 'folder_b'));
    fs.writeFileSync(path.join(VAULT_TREE_DIR, 'folder_b', 'note_b.md'), '# Note B');

    fs.mkdirSync(path.join(VAULT_TREE_DIR, 'folder_b', 'subfolder_b1'));
    fs.writeFileSync(path.join(VAULT_TREE_DIR, 'folder_b', 'subfolder_b1', 'nested_note.md'), '# Nested Note');

    // Create empty vault for empty vault test
    if (fs.existsSync(EMPTY_VAULT_DIR)) fs.rmSync(EMPTY_VAULT_DIR, { recursive: true, force: true });
    fs.mkdirSync(EMPTY_VAULT_DIR);
});

test('Task 2.1: File Tree Navigation', async ({ page }) => {
    console.log('Navigating to home page...');
    await page.goto('/');

    // Create/Select Vault
    console.log('Setting up vault for tree test...');
    const addVaultBtn = page.locator('#add-vault-btn');
    await addVaultBtn.waitFor({ state: 'attached', timeout: 10000 });
    await addVaultBtn.click();

    await page.fill('#vault-path', VAULT_TREE_DIR);
    await page.fill('#vault-name', 'Tree Vault');
    await page.click('#add-vault-form button[type="submit"]');

    // Wait for modal to close (or close it manually if it doesn't auto-close)
    const modal = page.locator('#add-vault-modal');
    try {
        await expect(modal).toBeHidden({ timeout: 2000 });
    } catch {
        // If modal didn't close, close it manually
        console.log('Modal did not auto-close, closing manually...');
        const closeBtn = modal.locator('button:has-text("✕")');
        if (await closeBtn.isVisible()) {
            await closeBtn.click();
        }
    }

    // Manually select the vault from dropdown if not auto-selected
    const vaultSelect = page.locator('#vault-select');
    const currentValue = await vaultSelect.inputValue();
    if (!currentValue || currentValue === '') {
        console.log('Vault not auto-selected, selecting manually...');
        await vaultSelect.selectOption({ label: 'Tree Vault' });
        await page.waitForTimeout(1000);
    }

    // 1. Verify Structure - wait for content to appear in file tree
    console.log('Verifying initial structure...');
    const fileTree = page.locator('#file-tree');
    await expect(fileTree).toContainText('root_note.md', { timeout: 10000 });
    await expect(fileTree).toContainText('folder_a');
    await expect(fileTree).toContainText('folder_b');

    // 2. Test Expand/Collapse
    console.log('Testing Expand/Collapse...');

    // Find the folder_a arrow (should be expanded by default showing ▼)
    const folderAArrow = page.locator('.file-tree-item.folder:has-text("folder_a") .folder-arrow').first();
    await expect(folderAArrow).toBeVisible();
    await expect(folderAArrow).toHaveText('▼');

    // Verify children are visible initially
    const folderANode = page.locator('.file-tree-node:has(.file-tree-item.folder:has-text("folder_a"))').first();
    await expect(folderANode.locator('.file-tree-children')).toBeVisible();
    await expect(folderANode.locator('text=note_a.md')).toBeVisible();

    // Click arrow to collapse
    console.log('Clicking to collapse folder_a...');
    await folderAArrow.click();

    // Verify it's collapsed (arrow shows ▶ and children are hidden)
    await expect(folderAArrow).toHaveText('▶');
    await expect(folderANode).toHaveClass(/collapsed/);
    await expect(folderANode.locator('.file-tree-children')).toBeHidden();

    // Click arrow to expand again
    console.log('Clicking to expand folder_a...');
    await folderAArrow.click();

    // Verify it's expanded again
    await expect(folderAArrow).toHaveText('▼');
    await expect(folderANode).not.toHaveClass(/collapsed/);
    await expect(folderANode.locator('.file-tree-children')).toBeVisible();
    await expect(folderANode.locator('text=note_a.md')).toBeVisible();

    // 3. Test Selection - Click a file
    console.log('Testing File Selection...');
    const noteA = page.locator('.file-tree-item:has-text("note_a.md")').first();
    await noteA.click();

    // Verify content opens in editor (give it time to load)
    await expect(page.locator('#pane-1')).toContainText('# Note A', { timeout: 5000 });
    console.log('File opened successfully');

    console.log('Test 2.1 completed successfully');
});

test('Task 2.1: Empty Vault', async ({ page }) => {
    console.log('Testing empty vault scenario...');
    await page.goto('/');

    // Create empty vault
    const addVaultBtn = page.locator('#add-vault-btn');
    await addVaultBtn.waitFor({ state: 'attached', timeout: 10000 });
    await addVaultBtn.click();

    await page.fill('#vault-path', EMPTY_VAULT_DIR);
    await page.fill('#vault-name', 'Empty Vault');
    await page.click('#add-vault-form button[type="submit"]');

    // Wait for modal to close and vault to be created
    await page.waitForTimeout(1000);

    // Manually select the vault from dropdown if not auto-selected
    const vaultSelect = page.locator('#vault-select');
    const currentValue = await vaultSelect.inputValue();
    if (!currentValue || currentValue === '') {
        console.log('Vault not auto-selected, selecting manually...');
        await vaultSelect.selectOption({ label: 'Empty Vault' });
        await page.waitForTimeout(1000);
    }

    // Verify "No files found" message appears
    console.log('Verifying "No files found" message...');
    const fileTree = page.locator('#file-tree');
    await expect(fileTree).toContainText('No files found', { timeout: 10000 });

    console.log('Empty vault test completed successfully');
});
