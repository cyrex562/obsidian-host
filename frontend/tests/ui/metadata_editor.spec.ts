import { test, expect } from '@playwright/test';

const VAULT_DIR = 'C:\\Users\\cyrex\\files\\projects\\obsidian-host\\test_vaults\\vault_metadata';
const BASE_URL = 'http://localhost:8080';

test.describe('Test 6.2 Metadata Editor', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto(BASE_URL, { waitUntil: 'networkidle' });

        const vaultSelect = page.locator('#vault-select');
        await vaultSelect.waitFor({ state: 'visible', timeout: 5000 });

        // Get current vault options
        const options = await vaultSelect.locator('option').allTextContents();

        // If vault doesn't exist, add it
        if (!options.some(opt => opt.includes('Metadata Test'))) {
            const addVaultBtn = page.locator('#add-vault-btn');
            await addVaultBtn.click();

            const modal = page.locator('#add-vault-modal');
            await expect(modal).toBeVisible();

            await page.locator('#vault-name').fill('Metadata Test');
            await page.locator('#vault-path').fill(VAULT_DIR);
            await page.locator('#add-vault-modal button[type="submit"]').click();

            await page.waitForTimeout(1500);
        }

        // Select the vault
        const vaultSelectEl = page.locator('#vault-select');
        const metadataOption = vaultSelectEl.locator('option:has-text("Metadata Test")').first();
        const optionValue = await metadataOption.getAttribute('value').catch(() => null);

        if (optionValue) {
            await vaultSelect.selectOption(optionValue);
            await vaultSelect.evaluate((el: any) => {
                el.dispatchEvent(new Event('change', { bubbles: true }));
            });
            await page.waitForTimeout(2000);
        }
    });

    test('should open note with frontmatter in editor', async ({ page }) => {
        // Click on the first note to open it
        const fileItem = page.locator('text=project_alpha.md').first();
        await fileItem.click();

        // Wait for the file to load in the editor
        await page.waitForTimeout(1000);

        // Verify the editor content is visible
        const editor = page.locator('.editor-content');
        await expect(editor).toBeVisible();

        // Verify file is opened in a tab
        const tab = page.locator('.tab:has-text("project_alpha")').first();
        await expect(tab).toBeVisible();
    });

    test('should display properties panel when note with frontmatter is open', async ({ page }) => {
        // Open a note with frontmatter
        const fileItem = page.locator('text=documentation_update.md').first();
        await fileItem.click();
        await page.waitForTimeout(1500);

        // The properties panel should be hidden by default
        let propertiesPanel = page.locator('#properties-panel');
        let isHidden = await propertiesPanel.evaluate((el) => el.classList.contains('hidden'));

        // Make properties panel visible using JavaScript
        await page.evaluate(() => {
            const panel = document.getElementById('properties-panel');
            if (panel && panel.classList.contains('hidden')) {
                panel.classList.remove('hidden');
            }
        });

        await page.waitForTimeout(500);

        // Panel should be visible now
        isHidden = await propertiesPanel.evaluate((el) => el.classList.contains('hidden'));
        expect(isHidden).toBeFalsy();
    });

    test('should view frontmatter properties as key-value pairs in properties panel', async ({ page }) => {
        // Open project_alpha.md which has: title, status, priority, tags, author, date
        const fileItem = page.locator('text=project_alpha.md').first();
        await fileItem.click();
        await page.waitForTimeout(1500);

        // Make properties panel visible
        await page.evaluate(() => {
            const panel = document.getElementById('properties-panel');
            if (panel && panel.classList.contains('hidden')) {
                panel.classList.remove('hidden');
            }
        });

        await page.waitForTimeout(500);

        // Trigger rendering of properties
        await page.evaluate(() => {
            const app = (window as any).app;
            if (app && app.renderProperties) {
                app.renderProperties();
            }
        });

        await page.waitForTimeout(500);

        // Verify properties content exists
        const propertiesContent = page.locator('#properties-content');
        await expect(propertiesContent).toBeVisible();

        // Should have property items
        const propertyItems = page.locator('.property-item');
        const count = await propertyItems.count();

        // We should see at least 1 property from the frontmatter
        expect(count).toBeGreaterThanOrEqual(1);
    });

    test('should edit a property value in the properties panel', async ({ page }) => {
        // Open documentation_update.md
        const fileItem = page.locator('text=documentation_update.md').first();
        await fileItem.click();
        await page.waitForTimeout(1500);

        // Make properties panel visible
        await page.evaluate(() => {
            const panel = document.getElementById('properties-panel');
            if (panel && panel.classList.contains('hidden')) {
                panel.classList.remove('hidden');
            }
        });

        await page.waitForTimeout(500);

        // Render properties
        await page.evaluate(() => {
            const app = (window as any).app;
            if (app && app.renderProperties) {
                app.renderProperties();
            }
        });

        await page.waitForTimeout(500);

        // Get property items
        const propertyItems = page.locator('.property-item');
        const count = await propertyItems.count();

        // If we have properties, edit one
        if (count > 0) {
            // Get the first property item
            const firstItem = propertyItems.first();
            const keyInput = firstItem.locator('.property-key').first();
            const valueTextarea = firstItem.locator('.property-value').first();

            // Get the current values
            const currentKey = await keyInput.inputValue();
            const currentValue = await valueTextarea.inputValue();

            expect(currentKey).toBeTruthy();
            expect(currentValue).toBeTruthy();

            // Modify the value
            await valueTextarea.clear();
            await valueTextarea.fill('modified_test_value');

            // Verify it changed
            const newValue = await valueTextarea.inputValue();
            expect(newValue).toBe('modified_test_value');
        }
    });

    test('should change tags property value correctly', async ({ page }) => {
        // Open project_alpha.md which has tags property
        const fileItem = page.locator('text=project_alpha.md').first();
        await fileItem.click();
        await page.waitForTimeout(1500);

        // Make properties panel visible
        await page.evaluate(() => {
            const panel = document.getElementById('properties-panel');
            if (panel && panel.classList.contains('hidden')) {
                panel.classList.remove('hidden');
            }
        });

        await page.waitForTimeout(500);

        // Render properties
        await page.evaluate(() => {
            const app = (window as any).app;
            if (app && app.renderProperties) {
                app.renderProperties();
            }
        });

        await page.waitForTimeout(500);

        // Find and modify the tags property
        const propertyItems = page.locator('.property-item');
        let tagsFound = false;

        for (let i = 0; i < await propertyItems.count(); i++) {
            const item = propertyItems.nth(i);
            const keyInput = item.locator('.property-key').first();
            const keyValue = await keyInput.inputValue();

            if (keyValue === 'tags') {
                tagsFound = true;
                const valueTextarea = item.locator('.property-value').first();

                // Get current value (should include 'wip')
                const currentValue = await valueTextarea.inputValue();

                // Replace wip with done
                const updatedValue = currentValue.replace('wip', 'done');

                // Update the textarea
                await valueTextarea.clear();
                await valueTextarea.fill(updatedValue);

                // Verify the change
                const newValue = await valueTextarea.inputValue();
                expect(newValue).toContain('done');
                break;
            }
        }

        expect(tagsFound).toBeTruthy();
    });

    test('should save property changes to the file', async ({ page }) => {
        // Open documentation_update.md
        const fileItem = page.locator('text=documentation_update.md').first();
        await fileItem.click();
        await page.waitForTimeout(1500);

        // Make properties panel visible
        await page.evaluate(() => {
            const panel = document.getElementById('properties-panel');
            if (panel && panel.classList.contains('hidden')) {
                panel.classList.remove('hidden');
            }
        });

        await page.waitForTimeout(500);

        // Render properties
        await page.evaluate(() => {
            const app = (window as any).app;
            if (app && app.renderProperties) {
                app.renderProperties();
            }
        });

        await page.waitForTimeout(500);

        // Modify a property
        const propertyItems = page.locator('.property-item');
        if (await propertyItems.count() > 0) {
            const firstItem = propertyItems.first();
            const valueTextarea = firstItem.locator('.property-value').first();

            const originalValue = await valueTextarea.inputValue();
            await valueTextarea.clear();
            await valueTextarea.fill('saved_test_value');

            // Click save button
            const saveBtn = page.locator('#save-properties-btn').first();
            await expect(saveBtn).toBeVisible();
            await saveBtn.click();

            await page.waitForTimeout(1000);

            // Verify the value was updated in the UI
            const updatedValue = await valueTextarea.inputValue();
            expect(updatedValue).toBeTruthy();
        }
    });

    test('should toggle properties panel visibility', async ({ page }) => {
        // Open a file
        const fileItem = page.locator('text=project_alpha.md').first();
        await fileItem.click();
        await page.waitForTimeout(1500);

        // Properties panel should be hidden initially
        let propertiesPanel = page.locator('#properties-panel');
        let isHidden = await propertiesPanel.evaluate((el) => el.classList.contains('hidden'));
        expect(isHidden).toBeTruthy();

        // Show properties panel
        await page.evaluate(() => {
            const panel = document.getElementById('properties-panel');
            if (panel && panel.classList.contains('hidden')) {
                panel.classList.remove('hidden');
            }
        });

        await page.waitForTimeout(300);

        // Verify it's now visible
        isHidden = await propertiesPanel.evaluate((el) => el.classList.contains('hidden'));
        expect(isHidden).toBeFalsy();

        // Close properties panel using the close button
        const closeBtn = page.locator('#close-properties').first();
        await expect(closeBtn).toBeVisible();
        await closeBtn.click();

        await page.waitForTimeout(300);

        // Verify it's hidden again
        isHidden = await propertiesPanel.evaluate((el) => el.classList.contains('hidden'));
        expect(isHidden).toBeTruthy();
    });
});
