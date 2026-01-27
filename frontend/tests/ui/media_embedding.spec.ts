import { test, expect } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

const VAULT_DIR = path.join(__dirname, '../../../vault_media_embedding');

test.describe('Test 3.3 Media Embedding', () => {
    test.beforeAll(async () => {
        // Create test vault if it doesn't exist
        if (!fs.existsSync(VAULT_DIR)) {
            fs.mkdirSync(VAULT_DIR, { recursive: true });
        }

        // Create a simple test image (1x1 PNG)
        // This is a minimal valid PNG file (1x1 transparent pixel)
        const pngData = Buffer.from([
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
            0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
            0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
            0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, // IDAT chunk
            0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
            0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, // IEND chunk
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
            0x42, 0x60, 0x82
        ]);
        fs.writeFileSync(path.join(VAULT_DIR, 'test-image.png'), pngData);

        // Create test markdown file
        fs.writeFileSync(path.join(VAULT_DIR, 'media-note.md'), '# Media Test\n\nThis is a test note.');
    });

    test.afterAll(async () => {
        // Clean up test vault
        if (fs.existsSync(VAULT_DIR)) {
            fs.rmSync(VAULT_DIR, { recursive: true, force: true });
        }
    });

    test('should render image embed in preview', async ({ page }) => {
        // Navigate to the app
        await page.goto('/');

        // Wait for vault selector
        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });

        // Check if vault already exists, if not add it
        const options = await vaultSelect.locator('option').allTextContents();
        if (!options.some(opt => opt.includes('Media Embedding Test'))) {
            const addVaultBtn = page.locator('#add-vault-btn');
            await addVaultBtn.click();

            const modal = page.locator('#add-vault-modal');
            await expect(modal).toBeVisible();

            await page.locator('#vault-name').fill('Media Embedding Test');
            await page.locator('#vault-path').fill(VAULT_DIR);
            await page.locator('#add-vault-modal button[type="submit"]').click();

            await page.waitForTimeout(500);
        }

        // Select the vault
        await vaultSelect.selectOption({ label: 'Media Embedding Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree to load
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('media-note.md', { timeout: 10000 });

        // Open the markdown file
        const markdownFile = page.locator('.file-tree-item').filter({ hasText: 'media-note.md' }).first();
        await markdownFile.click();
        await page.waitForTimeout(500);

        // Switch to side-by-side mode to see the preview
        const sideBySideBtn = page.locator('.mode-btn[data-mode="side-by-side"]');
        await sideBySideBtn.click();
        await page.waitForTimeout(500);

        // Type image embed syntax in the editor
        const textarea = page.locator('textarea.editor-raw');
        await textarea.fill('# Media Test\n\nHere is an image:\n\n![[test-image.png]]\n\nImage above.');
        await page.waitForTimeout(800); // Wait for debounced preview update

        // Verify image appears in the preview pane
        const previewPane = page.locator('.editor-preview');
        await expect(previewPane).toBeVisible();

        // Check for img element with wiki-embed class
        const imageEmbed = previewPane.locator('img.wiki-embed');
        await expect(imageEmbed).toBeVisible({ timeout: 3000 });

        // Verify the image has the correct data attribute
        const dataOriginalLink = await imageEmbed.getAttribute('data-original-link');
        expect(dataOriginalLink).toBe('test-image.png');

        // Verify the alt text is set
        const altText = await imageEmbed.getAttribute('alt');
        expect(altText).toBe('test-image.png');

        // Verify the src is pointing to the image (should contain the file path)
        const src = await imageEmbed.getAttribute('src');
        expect(src).toBeTruthy();
        expect(src).toContain('test-image.png');
    });

    test('should render image embed in formatted mode', async ({ page }) => {
        // Navigate and set up vault
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Media Embedding Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('media-note.md', { timeout: 10000 });

        // Open the markdown file
        const markdownFile = page.locator('.file-tree-item').filter({ hasText: 'media-note.md' }).first();
        await markdownFile.click();
        await page.waitForTimeout(500);

        // First type the content in raw mode
        const textarea = page.locator('textarea.editor-raw');
        await textarea.fill('# Media Test\n\nHere is an image:\n\n![[test-image.png]]');
        await page.waitForTimeout(500);

        // Switch to formatted mode to verify the syntax highlighting
        const formattedBtn = page.locator('.mode-btn[data-mode="formatted"]');
        await formattedBtn.click();
        await page.waitForTimeout(800);

        // Get the formatted editor container
        const formattedEditor = page.locator('.editor-formatted');
        await expect(formattedEditor).toBeVisible();

        // In formatted mode, the image syntax should be visible with highlighting
        const editorContent = await formattedEditor.textContent();
        expect(editorContent).toContain('![[test-image.png]]');
    });

    test('should handle image embed with alt text', async ({ page }) => {
        // Navigate and set up vault
        await page.goto('/');

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });
        await vaultSelect.selectOption({ label: 'Media Embedding Test' });
        await page.waitForTimeout(1000);

        // Wait for file tree
        const fileTree = page.locator('#file-tree');
        await expect(fileTree).toContainText('media-note.md', { timeout: 10000 });

        // Open the markdown file
        const markdownFile = page.locator('.file-tree-item').filter({ hasText: 'media-note.md' }).first();
        await markdownFile.click();
        await page.waitForTimeout(500);

        // Switch to side-by-side mode
        const sideBySideBtn = page.locator('.mode-btn[data-mode="side-by-side"]');
        await sideBySideBtn.click();
        await page.waitForTimeout(500);

        // Type image embed with alt text
        const textarea = page.locator('textarea.editor-raw');
        await textarea.fill('# Media Test\n\nImage with alt text:\n\n![[test-image.png|My Test Image]]\n');
        await page.waitForTimeout(800);

        // Verify image appears with custom alt text
        const previewPane = page.locator('.editor-preview');
        const imageEmbed = previewPane.locator('img.wiki-embed');
        await expect(imageEmbed).toBeVisible({ timeout: 3000 });

        // Verify the alt text is the custom text
        const altText = await imageEmbed.getAttribute('alt');
        expect(altText).toBe('My Test Image');
    });
});
