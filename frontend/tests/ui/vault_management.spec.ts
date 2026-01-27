import { test, expect } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

const FIXTURES_DIR = path.resolve(__dirname, '../../tests/fixtures');
const VAULT_1_DIR = path.join(FIXTURES_DIR, 'vault_1');
const VAULT_2_DIR = path.join(FIXTURES_DIR, 'vault_2');

test.beforeAll(async () => {
    // Setup test vaults
    if (!fs.existsSync(FIXTURES_DIR)) fs.mkdirSync(FIXTURES_DIR, { recursive: true });

    if (!fs.existsSync(VAULT_1_DIR)) fs.mkdirSync(VAULT_1_DIR);
    fs.writeFileSync(path.join(VAULT_1_DIR, 'note1.md'), '# Vault 1 Note');

    if (!fs.existsSync(VAULT_2_DIR)) fs.mkdirSync(VAULT_2_DIR);
    fs.writeFileSync(path.join(VAULT_2_DIR, 'note2.md'), '# Vault 2 Note');
});

test.afterAll(async () => {
    // Cleanup is optional, maybe we want to inspect
    // fs.rmSync(FIXTURES_DIR, { recursive: true, force: true });
});

test('Task 1.1: Vault Selection & Creation', async ({ page }) => {
    page.on('dialog', dialog => {
        console.log(`Dialog message: ${dialog.message()}`);
        dialog.accept();
    });

    console.log('Navigating to home page...');
    await page.goto('/');
    await expect(page).toHaveTitle(/Obsidian Host/i);

    // 1. Initial State
    console.log('Checking initial state...');
    const addVaultBtn = page.locator('#add-vault-btn');
    // Wait for it to be attached to DOM
    await addVaultBtn.waitFor({ state: 'attached', timeout: 10000 });

    // Check visibility logic - if it fails here, something is hiding it
    const isVisible = await addVaultBtn.isVisible();
    console.log(`Add Vault button visible: ${isVisible}`);
    if (!isVisible) {
        // Dump body HTML to see if something obscures it
        const bodyHtml = await page.innerHTML('body');
        console.log('Body HTML snapshot:', bodyHtml.slice(0, 1000));
    }
    await expect(addVaultBtn).toBeVisible();

    // 2. Add Vault 1
    console.log('Adding Vault 1...');
    await addVaultBtn.click();

    // Modal should open
    const modal = page.locator('#add-vault-modal');
    await expect(modal).toBeVisible();

    await page.fill('#vault-path', VAULT_1_DIR);
    await page.fill('#vault-name', 'Vault One');

    // Click submit in the form
    await page.click('#add-vault-form button[type="submit"]');

    // Verify Vault 1 is selected
    // The file tree should eventually show note1.md
    console.log('Verifying Vault 1 content...');
    await expect(page.locator('#file-tree')).toContainText('note1.md', { timeout: 10000 });
    await expect(page.locator('#file-tree')).not.toContainText('note2.md');

    // 3. Add Vault 2
    console.log('Adding Vault 2...');
    await addVaultBtn.click();

    await page.fill('#vault-path', VAULT_2_DIR);
    await page.fill('#vault-name', 'Vault Two');
    await page.click('#add-vault-form button[type="submit"]');

    console.log('Verifying Vault 2 content...');
    await expect(page.locator('#file-tree')).toContainText('note2.md', { timeout: 10000 });
    await expect(page.locator('#file-tree')).not.toContainText('note1.md');

    // 4. Switch Vault
    // Select Vault One from dropdown
    console.log('Switching back to Vault 1...');
    const vaultSelect = page.locator('#vault-select');
    // We need to know the value of the option. Usually IDs or names.
    // We can select by label.
    await vaultSelect.selectOption({ label: 'Vault One' });

    console.log('Verifying Vault 1 content after switch...');
    await expect(page.locator('#file-tree')).toContainText('note1.md', { timeout: 10000 });
    await expect(page.locator('#file-tree')).not.toContainText('note2.md');

    // Switch to Vault Two
    console.log('Switching to Vault 2...');
    await vaultSelect.selectOption({ label: 'Vault Two' });

    console.log('Verifying Vault 2 content after switch...');
    await expect(page.locator('#file-tree')).toContainText('note2.md', { timeout: 10000 });
    await expect(page.locator('#file-tree')).not.toContainText('note1.md');

    console.log('Test Complete.');
});
