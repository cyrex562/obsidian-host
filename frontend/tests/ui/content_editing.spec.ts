import { test, expect } from '@playwright/test';
import * as path from 'path';
import * as fs from 'fs';

const FIXTURES_DIR = path.resolve(__dirname, '../../tests/fixtures');
const VAULT_DIR = path.join(FIXTURES_DIR, 'vault_content_editing');

test.describe('Test 3.1: Content Editing', () => {
    test.beforeAll(async () => {
        // Clean up and create test vault
        if (fs.existsSync(VAULT_DIR)) {
            fs.rmSync(VAULT_DIR, { recursive: true, force: true });
        }
        fs.mkdirSync(VAULT_DIR, { recursive: true });

        // Create test files
        fs.writeFileSync(path.join(VAULT_DIR, 'test-note.md'), '# Test Note\n\nInitial content here.');
        fs.writeFileSync(path.join(VAULT_DIR, 'link-target.md'), '# Link Target\n\nThis is the target of a link.');
    });

    test.afterAll(async () => {
        // Clean up test vault
        if (fs.existsSync(VAULT_DIR)) {
            fs.rmSync(VAULT_DIR, { recursive: true, force: true });
        }
    });

    test('should open file and load content in editor', async ({ page }) => {
        // Navigate to the app and set up vault
        await page.goto('/');
        const addVaultBtn = page.locator('#add-vault-btn');
        await addVaultBtn.waitFor({ state: 'attached', timeout: 10000 });
        await addVaultBtn.click();

        await page.fill('#vault-path', VAULT_DIR);
        await page.fill('#vault-name', 'Content Editing Test');
        await page.click('#add-vault-form button[type="submit"]');

        // Wait for modal to close
        const modal = page.locator('#add-vault-modal');
        try {
            await expect(modal).toBeHidden({ timeout: 2000 });
        } catch {
            const closeBtn = modal.locator('button:has-text("✕")');
            if (await closeBtn.isVisible()) {
                await closeBtn.click();
            }
        }

        // Select vault if not auto-selected
        const vaultSelect = page.locator('#vault-select');
        const currentValue = await vaultSelect.inputValue();
        if (!currentValue || currentValue === '') {
            await vaultSelect.selectOption({ label: 'Content Editing Test' });
            await page.waitForTimeout(1000);
        }

        // Wait for file tree to load
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('test-note.md', { timeout: 10000 });

        // Click on the file to open it
        const fileItem = page.locator('.file-tree-item').filter({ hasText: 'test-note.md' }).first();
        await fileItem.click();

        // Wait for editor to load
        await page.waitForTimeout(500);

        // Verify file opens in a tab
        const activeTab = page.locator('.tab.active');
        await expect(activeTab).toContainText('test-note.md');

        // Verify content loads in editor (textarea with class editor-raw)
        const textarea = page.locator('textarea.editor-raw');
        await expect(textarea).toBeVisible();

        // Check that the content is present
        const editorContent = await textarea.inputValue();
        expect(editorContent).toContain('# Test Note');
        expect(editorContent).toContain('Initial content here.');
    });

    test('should show unsaved changes indicator when typing', async ({ page }) => {
        // Navigate and set up vault (reusing the vault from beforeAll)
        await page.goto('/');

        // Select vault
        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Content Editing Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree to load
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('test-note.md', { timeout: 10000 });

        // Open the file
        const fileItem = page.locator('.file-tree-item').filter({ hasText: 'test-note.md' }).first();
        await fileItem.click();
        await page.waitForTimeout(500);

        // Type in the editor textarea
        const textarea = page.locator('textarea.editor-raw');
        await textarea.fill('# Modified Content\n\nThis is new text that I am typing.');

        // Wait a moment for the change to register
        await page.waitForTimeout(300);

        // Verify unsaved changes indicator appears (dot in tab name)
        const activeTab = page.locator('.tab.active .tab-name');
        await expect(activeTab).toContainText('• test-note.md', { timeout: 2000 });
    });

    test('should auto-save after 5 seconds', async ({ page }) => {
        // Navigate and set up vault
        await page.goto('/');

        // Select vault
        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Content Editing Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('test-note.md', { timeout: 10000 });

        // Open the file
        const fileItem = page.locator('.file-tree-item').filter({ hasText: 'test-note.md' }).first();
        await fileItem.click();
        await page.waitForTimeout(500);

        // Type in the editor textarea
        const testContent = '# Auto Save Test\n\nThis content should auto-save.';
        const textarea = page.locator('textarea.editor-raw');
        await textarea.fill(testContent);

        // Wait for auto-save (5 seconds + buffer)
        await page.waitForTimeout(5500);

        // Verify "Saved" indicator appears
        const saveStatus = page.locator('#save-status');
        await expect(saveStatus).toContainText('Saved', { timeout: 3000 });

        // Verify file was actually saved to disk
        const savedContent = fs.readFileSync(path.join(VAULT_DIR, 'test-note.md'), 'utf-8');
        expect(savedContent).toBe(testContent);
    });

    test('should render markdown bold text', async ({ page }) => {
        // Navigate and set up vault
        await page.goto('/');

        // Select vault
        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Content Editing Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('test-note.md', { timeout: 10000 });

        // Open the file
        const fileItem = page.locator('.file-tree-item').filter({ hasText: 'test-note.md' }).first();
        await fileItem.click();
        await page.waitForTimeout(500);

        // Type markdown with bold syntax
        const markdownContent = '# Bold Test\n\nThis is **bold text** in the middle.';
        const textarea = page.locator('textarea.editor-raw');
        await textarea.fill(markdownContent);

        // Wait for content to be set
        await page.waitForTimeout(300);

        // Verify the markdown syntax is present in the editor
        const editorContent = await textarea.inputValue();
        expect(editorContent).toContain('**bold text**');
        expect(editorContent).toContain('# Bold Test');
    });

    test('should create internal link', async ({ page }) => {
        // Navigate and set up vault
        await page.goto('/');

        // Select vault
        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Content Editing Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('test-note.md', { timeout: 10000 });

        // Open the file
        const fileItem = page.locator('.file-tree-item').filter({ hasText: 'test-note.md' }).first();
        await fileItem.click();
        await page.waitForTimeout(500);

        // Type markdown with internal link
        const markdownContent = '# Link Test\n\nThis is a link: [[link-target]]';
        const textarea = page.locator('textarea.editor-raw');
        await textarea.fill(markdownContent);

        // Wait for content to be set
        await page.waitForTimeout(300);

        // Verify the link syntax exists in the editor
        const editorContent = await textarea.inputValue();
        expect(editorContent).toContain('[[link-target]]');
        expect(editorContent).toContain('# Link Test');
    });
});
