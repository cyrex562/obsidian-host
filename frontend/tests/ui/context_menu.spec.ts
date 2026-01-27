import { test, expect } from '@playwright/test';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';

const FIXTURES_DIR = path.resolve(__dirname, '../../tests/fixtures');
const VAULT_DIR = path.join(FIXTURES_DIR, 'vault_context_menu');

test.describe('Test 2.2: Context Menu Actions', () => {
    test.beforeAll(async () => {
        // Clean up and create test vault
        if (fs.existsSync(VAULT_DIR)) {
            fs.rmSync(VAULT_DIR, { recursive: true, force: true });
        }
        fs.mkdirSync(VAULT_DIR, { recursive: true });

        // Create test structure
        fs.mkdirSync(path.join(VAULT_DIR, 'Folder1'), { recursive: true });
        fs.writeFileSync(path.join(VAULT_DIR, 'note.md'), '# Test Note\n\nContent here.');
        fs.writeFileSync(path.join(VAULT_DIR, 'Folder1', 'existing.md'), '# Existing\n');
    });

    test.afterAll(async () => {
        // Clean up test vault
        if (fs.existsSync(VAULT_DIR)) {
            fs.rmSync(VAULT_DIR, { recursive: true, force: true });
        }
    });

    test('should create new file via context menu', async ({ page }) => {
        // Navigate to the app and set up vault
        await page.goto('/');
        const addVaultBtn = page.locator('#add-vault-btn');
        await addVaultBtn.waitFor({ state: 'attached', timeout: 10000 });
        await addVaultBtn.click();

        await page.fill('#vault-path', VAULT_DIR);
        await page.fill('#vault-name', 'Context Menu Test');
        await page.click('#add-vault-form button[type="submit"]');

        // Wait for modal to close (or close it manually if it doesn't auto-close)
        const modal = page.locator('#add-vault-modal');
        try {
            await expect(modal).toBeHidden({ timeout: 2000 });
        } catch {
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
            await vaultSelect.selectOption({ label: 'Context Menu Test' });
            await page.waitForTimeout(1000);
        }

        // Wait for file tree to load
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('Folder1', { timeout: 10000 });

        // Right-click on Folder1 - click on the file-tree-item, not the wrapper
        const folder = page.locator('.file-tree-item.folder').filter({ hasText: 'Folder1' }).first();
        await folder.click({ button: 'right' });

        // Verify context menu appears
        const contextMenu = page.locator('.context-menu');
        await expect(contextMenu).toBeVisible();

        // Verify "New File" option exists
        const newFileOption = contextMenu.locator('.context-menu-item[data-action="new-file"]');
        await expect(newFileOption).toBeVisible();
        await expect(newFileOption).toHaveText('New File');

        // Click "New File" and handle prompt
        page.once('dialog', async dialog => {
            expect(dialog.type()).toBe('prompt');
            expect(dialog.message()).toContain('Enter file name');
            await dialog.accept('new-test.md');
        });
        await newFileOption.click();

        // Wait for file tree to update
        await page.waitForTimeout(1000);

        // Verify the file appears in the tree
        const newFile = page.locator('.file-tree-item').filter({ hasText: 'new-test.md' });
        await expect(newFile).toBeVisible();

        // Verify the file opens in editor
        const activeTab = page.locator('.tab.active');
        await expect(activeTab).toContainText('new-test.md');

        // Verify file exists on disk
        const newFilePath = path.join(VAULT_DIR, 'Folder1', 'new-test.md');
        expect(fs.existsSync(newFilePath)).toBeTruthy();
    });

    test('should create new folder via context menu', async ({ page }) => {
        // Navigate to the app
        await page.goto('/');

        // Select the vault
        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.selectOption({ label: 'Context Menu Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree to load
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('Folder1', { timeout: 10000 });

        // Right-click on Folder1
        const folder = page.locator('.file-tree-item.folder').filter({ hasText: 'Folder1' }).first();
        await folder.click({ button: 'right' });

        // Verify context menu appears
        const contextMenu = page.locator('.context-menu');
        await expect(contextMenu).toBeVisible();

        // Verify "New Folder" option exists
        const newFolderOption = contextMenu.locator('.context-menu-item[data-action="new-folder"]');
        await expect(newFolderOption).toBeVisible();
        await expect(newFolderOption).toHaveText('New Folder');

        // Click "New Folder" and handle prompt
        page.once('dialog', async dialog => {
            expect(dialog.type()).toBe('prompt');
            expect(dialog.message()).toContain('Enter folder name');
            await dialog.accept('SubFolder');
        });
        await newFolderOption.click();

        // Wait for file tree to update
        await page.waitForTimeout(1000);

        // Expand Folder1 if not already expanded
        const folderArrow = page.locator('.file-tree-item.folder').filter({ hasText: 'Folder1' }).locator('.folder-arrow').first();
        const arrowText = await folderArrow.textContent();
        if (arrowText === '▶') {
            await folderArrow.click();
            await page.waitForTimeout(300);
        }

        // Verify the folder appears in the tree
        const newFolder = page.locator('.file-tree-item.folder').filter({ hasText: 'SubFolder' });
        await expect(newFolder).toBeVisible();

        // Verify folder exists on disk
        const newFolderPath = path.join(VAULT_DIR, 'Folder1', 'SubFolder');
        expect(fs.existsSync(newFolderPath)).toBeTruthy();
        expect(fs.statSync(newFolderPath).isDirectory()).toBeTruthy();
    });

    test('should rename file via context menu', async ({ page }) => {
        // Navigate to the app
        await page.goto('/');

        // Select the vault
        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.selectOption({ label: 'Context Menu Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree to load
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('note.md', { timeout: 10000 });

        // Right-click on note.md
        const noteFile = page.locator('.file-tree-item').filter({ hasText: 'note.md' }).first();
        await noteFile.click({ button: 'right' });

        // Verify context menu appears
        const contextMenu = page.locator('.context-menu');
        await expect(contextMenu).toBeVisible();

        // Verify "Rename" option exists
        const renameOption = contextMenu.locator('.context-menu-item[data-action="rename"]');
        await expect(renameOption).toBeVisible();
        await expect(renameOption).toHaveText('Rename');

        // Click "Rename" and handle prompt
        page.once('dialog', async dialog => {
            expect(dialog.type()).toBe('prompt');
            expect(dialog.message()).toContain('Enter new name');
            await dialog.accept('renamed.md');
        });
        await renameOption.click();

        // Wait for file tree to update
        await page.waitForTimeout(1000);

        // Verify the old file name is gone
        const oldFile = page.locator('.file-tree-item').filter({ hasText: /^.*note\.md$/ });
        await expect(oldFile).not.toBeVisible();

        // Verify the new file name appears
        const renamedFile = page.locator('.file-tree-item').filter({ hasText: 'renamed.md' });
        await expect(renamedFile).toBeVisible();

        // Verify file exists on disk with new name
        const renamedPath = path.join(VAULT_DIR, 'renamed.md');
        expect(fs.existsSync(renamedPath)).toBeTruthy();

        const oldPath = path.join(VAULT_DIR, 'note.md');
        expect(fs.existsSync(oldPath)).toBeFalsy();
    });

    test('should delete file via context menu', async ({ page }) => {
        // Navigate to the app
        await page.goto('/');

        // Select the vault
        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.selectOption({ label: 'Context Menu Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree to load
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('renamed.md', { timeout: 10000 });

        // Right-click on renamed.md (from previous test)
        const renamedFile = page.locator('.file-tree-item').filter({ hasText: 'renamed.md' }).first();
        await renamedFile.click({ button: 'right' });

        // Verify context menu appears
        const contextMenu = page.locator('.context-menu');
        await expect(contextMenu).toBeVisible();

        // Verify "Delete" option exists
        const deleteOption = contextMenu.locator('.context-menu-item[data-action="delete"]');
        await expect(deleteOption).toBeVisible();
        await expect(deleteOption).toHaveText('Delete');

        // Click "Delete" and handle confirmation dialog
        page.once('dialog', async dialog => {
            expect(dialog.type()).toBe('confirm');
            expect(dialog.message()).toContain('Are you sure you want to delete');
            await dialog.accept();
        });
        await deleteOption.click();

        // Wait for file tree to update
        await page.waitForTimeout(1000);

        // Verify the file is gone from tree
        const deletedFile = page.locator('.file-tree-item').filter({ hasText: 'renamed.md' });
        await expect(deletedFile).not.toBeVisible();

        // Verify file no longer exists on disk
        const deletedPath = path.join(VAULT_DIR, 'renamed.md');
        expect(fs.existsSync(deletedPath)).toBeFalsy();
    });
});

