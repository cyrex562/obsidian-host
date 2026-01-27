import { test, expect } from '@playwright/test';

const DARK_BG = 'rgb(17, 17, 17)';
const LIGHT_BG = 'rgb(255, 255, 255)';

test.describe('Test 8.1 Dark/Light Mode', () => {
    test.beforeEach(async ({ request }) => {
        await request.post('/api/preferences/reset');
    });

    test('toggles theme and persists after reload', async ({ page }) => {
        await page.goto('/');

        // Wait for preferences to load
        await page.waitForResponse(response => response.url().includes('/api/preferences') && response.request().method() === 'GET');

        const body = page.locator('body');
        const themeToggle = page.locator('#theme-toggle-btn');
        await expect(themeToggle).toBeVisible();

        // Verify default dark theme
        await expect(body).toHaveClass(/theme-dark/);
        await expect.poll(async () => page.evaluate(() => getComputedStyle(document.body).backgroundColor)).toBe(DARK_BG);

        // Toggle to light and wait for preference save
        const savePrefs = page.waitForResponse(response => response.url().includes('/api/preferences') && response.request().method() === 'PUT');
        await themeToggle.click();
        await savePrefs;

        await expect(body).toHaveClass(/theme-light/);
        await expect.poll(async () => page.evaluate(() => getComputedStyle(document.body).backgroundColor)).toBe(LIGHT_BG);

        // Reload and confirm persistence
        await page.reload();
        await page.waitForResponse(response => response.url().includes('/api/preferences') && response.request().method() === 'GET');
        await expect(body).toHaveClass(/theme-light/);
        await expect.poll(async () => page.evaluate(() => getComputedStyle(document.body).backgroundColor)).toBe(LIGHT_BG);
    });
});
