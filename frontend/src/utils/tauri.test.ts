import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  isTauri,
  openDirectoryDialog,
  openFileDialog,
  saveFileDialog,
} from './tauri';

// Mock the Tauri dialog plugin (dynamic import inside tauri.ts).
// vi.mock is hoisted by Vitest, so it runs before any imports.
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

// ── Helpers ────────────────────────────────────────────────────────────────

async function getTauriDialogMock() {
  return await import('@tauri-apps/plugin-dialog') as unknown as {
    open: ReturnType<typeof vi.fn>;
    save: ReturnType<typeof vi.fn>;
  };
}

function setTauriContext(active: boolean) {
  if (active) {
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      value: {},
      writable: true,
      configurable: true,
    });
  } else {
    try { delete (window as unknown as Record<string, unknown>)['__TAURI_INTERNALS__']; } catch { /* noop */ }
    try { delete (window as unknown as Record<string, unknown>)['__TAURI__']; } catch { /* noop */ }
  }
}

beforeEach(() => {
  setTauriContext(false);
  vi.clearAllMocks();
});
afterEach(() => setTauriContext(false));

// ── isTauri ───────────────────────────────────────────────────────────────

describe('isTauri', () => {
  it('returns false in a plain browser environment', () => {
    expect(isTauri()).toBe(false);
  });

  it('returns true when __TAURI_INTERNALS__ is injected', () => {
    setTauriContext(true);
    expect(isTauri()).toBe(true);
  });

  it('returns true when __TAURI__ is present (Tauri v1 compatibility)', () => {
    Object.defineProperty(window, '__TAURI__', {
      value: {},
      writable: true,
      configurable: true,
    });
    expect(isTauri()).toBe(true);
  });
});

// ── openDirectoryDialog ───────────────────────────────────────────────────

describe('openDirectoryDialog', () => {
  it('returns null in a browser context (not Tauri)', async () => {
    expect(await openDirectoryDialog()).toBeNull();
  });

  it('returns the selected path in a Tauri context', async () => {
    setTauriContext(true);
    const { open } = await getTauriDialogMock();
    open.mockResolvedValue('/home/user/my-vault');

    expect(await openDirectoryDialog()).toBe('/home/user/my-vault');
    expect(open).toHaveBeenCalledWith(
      expect.objectContaining({ directory: true }),
    );
  });

  it('returns null when the user cancels the dialog', async () => {
    setTauriContext(true);
    const { open } = await getTauriDialogMock();
    open.mockResolvedValue(null);

    expect(await openDirectoryDialog()).toBeNull();
  });

  it('returns null when the plugin throws (graceful error handling)', async () => {
    setTauriContext(true);
    const { open } = await getTauriDialogMock();
    open.mockRejectedValue(new Error('permission denied'));

    expect(await openDirectoryDialog()).toBeNull();
  });
});

// ── openFileDialog ────────────────────────────────────────────────────────

describe('openFileDialog', () => {
  it('returns null in browser context', async () => {
    expect(await openFileDialog(['md'])).toBeNull();
  });

  it('returns the selected file path in Tauri context', async () => {
    setTauriContext(true);
    const { open } = await getTauriDialogMock();
    open.mockResolvedValue('/docs/note.md');

    const result = await openFileDialog(['md', 'txt']);
    expect(result).toBe('/docs/note.md');
  });

  it('passes extension filters to the Tauri open() call', async () => {
    setTauriContext(true);
    const { open } = await getTauriDialogMock();
    open.mockResolvedValue('/docs/note.md');

    await openFileDialog(['md', 'txt']);
    expect(open).toHaveBeenCalledWith(
      expect.objectContaining({
        filters: [{ name: 'Files', extensions: ['md', 'txt'] }],
        directory: false,
        multiple: false,
      }),
    );
  });

  it('omits filters when no extensions are provided', async () => {
    setTauriContext(true);
    const { open } = await getTauriDialogMock();
    open.mockResolvedValue('/docs/file.txt');

    await openFileDialog();
    expect(open).toHaveBeenCalledWith(
      expect.objectContaining({ filters: undefined }),
    );
  });
});

// ── saveFileDialog ────────────────────────────────────────────────────────

describe('saveFileDialog', () => {
  it('returns null in browser context', async () => {
    expect(await saveFileDialog('output.md')).toBeNull();
  });

  it('returns the chosen save path in Tauri context', async () => {
    setTauriContext(true);
    const { save } = await getTauriDialogMock();
    save.mockResolvedValue('/home/user/export.md');

    const result = await saveFileDialog('export.md', ['md']);
    expect(result).toBe('/home/user/export.md');
    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({
        defaultPath: 'export.md',
        filters: [{ name: 'Files', extensions: ['md'] }],
      }),
    );
  });

  it('returns null when the save dialog is cancelled', async () => {
    setTauriContext(true);
    const { save } = await getTauriDialogMock();
    save.mockResolvedValue(null);

    expect(await saveFileDialog('note.md')).toBeNull();
  });
});
