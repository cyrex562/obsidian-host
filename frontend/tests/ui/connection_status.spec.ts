import { test, expect } from '@playwright/test';
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

test('Task 1.2: Connection Status', async ({ page }) => {
    console.log('Navigating to home page...');
    await page.goto('/');
    await expect(page).toHaveTitle(/Obsidian Host/i);

    // 1. Online State - Verify green dot / "Connected" status when server is running
    console.log('Checking online state...');
    const connectionStatus = page.locator('#connection-status');

    // Wait for connection to establish
    await page.waitForTimeout(2000);

    // Verify the connection status element exists
    await expect(connectionStatus).toBeVisible();

    // Verify it shows "Connected" title
    await expect(connectionStatus).toHaveAttribute('title', 'Connected');

    // Verify it has the correct class
    await expect(connectionStatus).toHaveClass(/connection-connected/);

    // Verify green color (rgb(74, 222, 128) is #4ade80)
    const greenColor = await connectionStatus.evaluate((el) => {
        return window.getComputedStyle(el).color;
    });
    console.log(`Connected color: ${greenColor}`);
    expect(greenColor).toContain('74'); // Green color component

    // 2. Offline State - Stop server and verify red dot / "Disconnected" status appears
    console.log('Testing offline state - stopping server...');

    // Stop the server process
    try {
        await execAsync('Stop-Process -Name obsidian-host -Force -ErrorAction SilentlyContinue', { shell: 'powershell.exe' });
        console.log('Server stopped');
    } catch (error) {
        console.log('Server stop command executed (may have already been stopped)');
    }

    // Wait a moment for the process to stop
    await page.waitForTimeout(1000);

    // Wait for disconnection to be detected (within 5 seconds)
    await expect(connectionStatus).toHaveAttribute('title', /Disconnect|Reconnecting/, { timeout: 5000 });

    console.log('Disconnection detected successfully');

    // Verify the status shows disconnected or reconnecting
    const disconnectedTitle = await connectionStatus.getAttribute('title');
    console.log(`Status after disconnect: ${disconnectedTitle}`);

    // 3. Reconnection - Restart server and verify status returns to green automatically
    console.log('Testing reconnection - restarting server...');

    // Restart the server in a separate PowerShell window
    try {
        await execAsync(
            'Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd C:\\Users\\cyrex\\files\\projects\\obsidian-host; cargo run"',
            { shell: 'powershell.exe' }
        );
        console.log('Server restart command issued');
    } catch (error) {
        console.error('Error restarting server:', error);
    }

    // Wait for automatic reconnection (the app should attempt to reconnect)
    // Give it more time for cargo to compile and start
    await expect(connectionStatus).toHaveAttribute('title', 'Connected', { timeout: 30000 });
    console.log('Reconnection successful');

    // Verify it's back to green/connected state
    await expect(connectionStatus).toHaveClass(/connection-connected/);

    const reconnectedColor = await connectionStatus.evaluate((el) => {
        return window.getComputedStyle(el).color;
    });
    console.log(`Reconnected color: ${reconnectedColor}`);
    expect(reconnectedColor).toContain('74'); // Green color component

    console.log('Test Complete.');
});
