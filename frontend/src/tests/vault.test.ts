
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { UIManager, AppState, ApiClient } from '../app';

// Mock the problematic dependencies
vi.mock('../vendor/codejar/codejar.js', () => ({
    CodeJar: vi.fn()
}));

// Mock basic DOM elements
function createMockDOM() {
    document.body.innerHTML = `
        <select id="vault-select">
            <option value="">Select a vault...</option>
        </select>
        <button id="add-vault-btn"></button>
        <div id="add-vault-modal" class="modal hidden"></div>
        <form id="add-vault-form">
            <input name="name" value="Test Vault" />
            <input name="path" value="/tmp/test" />
        </form>
            <div id="vault-list"></div>
            <button id="vault-path-browse"></button>
            <input id="vault-path-picker" />
        <div id="file-tree"></div>
        <div id="quick-switcher-modal"></div>
        <input id="quick-switcher-input" />
        <div id="quick-switcher-results"></div>
    `;
}

describe('Vault Management UI', () => {
    let ui: UIManager;
    let apiMock: ApiClient;
    let state: AppState;

    beforeEach(() => {
        createMockDOM();
        state = new AppState();
        apiMock = new ApiClient();

        // Mock API methods
        apiMock.getVaults = vi.fn().mockResolvedValue([
            { id: 'v1', name: 'Existing Vault', path: '/path/v1', path_exists: true }
        ]);
        apiMock.createVault = vi.fn().mockResolvedValue({
            id: 'v2', name: 'Test Vault', path: '/tmp/test', path_exists: true
        });

        ui = new UIManager(state, apiMock);
        // Suppress console logs during tests
        console.log = vi.fn();
    });

    it('should load vaults into dropdown on init', async () => {
        await ui.loadVaults();

        const select = document.getElementById('vault-select') as HTMLSelectElement;
        expect(select.options.length).toBe(2); // Default option + 1 Loaded vault
        expect(select.options[1].text).toBe('Existing Vault');
        expect(select.options[1].value).toBe('v1');
    });

    it('should switch vault when dropdown changes', async () => {
        // First load existing vaults
        await ui.loadVaults();

        // Mock loadFileTree since switching vault calls it
        ui.loadFileTree = vi.fn().mockResolvedValue(undefined);
        ui.loadPlugins = vi.fn().mockResolvedValue(undefined); // Mock plugin loading

        // Simulate change event
        const select = document.getElementById('vault-select') as HTMLSelectElement;
        select.value = 'v1';

        // Trigger the logic manually since we can't easily trigger native events that attached listeners catch 
        // without calling setupEventListeners first. 
        // Let's call setupEventListeners once.
        ui.setupEventListeners();

        select.dispatchEvent(new Event('change'));

        expect(state.currentVaultId).toBe('v1');
        expect(ui.loadFileTree).toHaveBeenCalled();
    });

    it('should create a new vault', async () => {
        ui.setupEventListeners();

        // Simulate Add Vault Button Click to open modal
        const addBtn = document.getElementById('add-vault-btn') as HTMLElement;
        addBtn.click();

        const modal = document.getElementById('add-vault-modal');
        expect(modal?.classList.contains('hidden')).toBe(false);

        // Simulate Form Submit
        const form = document.getElementById('add-vault-form') as HTMLFormElement;
        form.dispatchEvent(new Event('submit'));

        // Wait for async operations
        await new Promise(resolve => setTimeout(resolve, 10));

        expect(apiMock.createVault).toHaveBeenCalledWith('Test Vault', '/tmp/test');
        expect(apiMock.getVaults).toHaveBeenCalledTimes(1); // loadVaults called again
    });
});
