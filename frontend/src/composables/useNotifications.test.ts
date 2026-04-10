import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { fireNotification, useNotifications } from './useNotifications';

// ── Mock Tauri internals ──────────────────────────────────────────────────────

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn().mockResolvedValue(undefined),
}));

// ── Helper ────────────────────────────────────────────────────────────────────

function setTauriEnv(active: boolean) {
    if (active) {
        (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    } else {
        delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
        delete (window as unknown as Record<string, unknown>).__TAURI__;
    }
}

type NotifiedEntry = { title: string; body: string | undefined };

function stubNotification(permission: NotificationPermission): {
    notified: NotifiedEntry[];
    requestPermission: ReturnType<typeof vi.fn>;
    restore: () => void;
} {
    const notified: NotifiedEntry[] = [];
    const requestPermission = vi.fn().mockResolvedValue(permission);

    class FakeNotification {
        static permission = permission;
        static requestPermission = requestPermission;
        constructor(title: string, opts?: NotificationOptions) {
            notified.push({ title, body: opts?.body });
        }
    }

    vi.stubGlobal('Notification', FakeNotification);

    return {
        notified,
        requestPermission,
        restore: () => vi.unstubAllGlobals(),
    };
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('fireNotification — browser context', () => {
    beforeEach(() => setTauriEnv(false));
    afterEach(() => vi.unstubAllGlobals());

    it('does nothing when Notification API is absent', async () => {
        vi.stubGlobal('Notification', undefined);
        await expect(
            fireNotification({ title: 'Test', body: 'body', channel: 'reindex' }),
        ).resolves.toBeUndefined();
    });

    it('requests permission when status is default', async () => {
        const { requestPermission } = stubNotification('denied');
        // Override permission to 'default' so requestPermission is called.
        Object.defineProperty(globalThis.Notification, 'permission', { value: 'default', writable: true, configurable: true });
        await fireNotification({ title: 'Test', body: 'body', channel: 'reindex' });
        expect(requestPermission).toHaveBeenCalled();
    });

    it('creates Notification when permission is granted', async () => {
        const { notified } = stubNotification('granted');
        await fireNotification({ title: 'Hello', body: 'World', channel: 'reindex' });
        expect(notified).toHaveLength(1);
        expect(notified[0].title).toBe('Hello');
        expect(notified[0].body).toBe('World');
    });

    it('does NOT create Notification when permission is denied', async () => {
        const { notified } = stubNotification('denied');
        await fireNotification({ title: 'Hello', body: 'World', channel: 'error' });
        expect(notified).toHaveLength(0);
    });
});

describe('fireNotification — Tauri context', () => {
    beforeEach(() => setTauriEnv(true));
    afterEach(() => {
        setTauriEnv(false);
        vi.clearAllMocks();
    });

    it('calls invoke("notify") with title and body', async () => {
        const { invoke } = await import('@tauri-apps/api/core');
        (invoke as ReturnType<typeof vi.fn>).mockClear();

        await fireNotification({ title: 'Reindex done', body: '10 files', channel: 'reindex' });

        expect(invoke).toHaveBeenCalledWith('notify', {
            title: 'Reindex done',
            body: '10 files',
        });
    });

    it('resolves without throwing when invoke rejects', async () => {
        const { invoke } = await import('@tauri-apps/api/core');
        (invoke as ReturnType<typeof vi.fn>).mockRejectedValueOnce(new Error('denied'));

        await expect(
            fireNotification({ title: 'Oops', body: 'fail', channel: 'error' }),
        ).resolves.toBeUndefined();
    });
});

describe('useNotifications — handleWsMessage', () => {
    beforeEach(() => setTauriEnv(false));
    afterEach(() => vi.unstubAllGlobals());

    it('fires a notification for ReindexComplete', async () => {
        const { notified } = stubNotification('granted');
        const { handleWsMessage } = useNotifications();
        handleWsMessage({ type: 'ReindexComplete', vault_id: 'v1', file_count: 42, duration_ms: 1500 });
        await new Promise((r) => setTimeout(r, 10));

        expect(notified.length).toBeGreaterThan(0);
        expect(notified[0].title).toBe('Reindex complete');
        expect(notified[0].body).toContain('42 files');
        expect(notified[0].body).toContain('1.5s');
    });

    it('fires a notification for Error events', async () => {
        const { notified } = stubNotification('granted');
        const { handleWsMessage } = useNotifications();
        handleWsMessage({ type: 'Error', message: 'sync failed' });
        await new Promise((r) => setTimeout(r, 10));

        expect(notified.length).toBeGreaterThan(0);
        expect(notified[0].title).toBe('Codex error');
        expect(notified[0].body).toBe('sync failed');
    });

    it('does not fire for FileChanged events', async () => {
        const { notified } = stubNotification('granted');
        const { handleWsMessage } = useNotifications();
        handleWsMessage({ type: 'FileChanged', vault_id: 'v1', path: 'a.md', event_type: 'Modified', timestamp: 0 });
        await new Promise((r) => setTimeout(r, 10));
        expect(notified.length).toBe(0);
    });

    it('does not fire for SyncPing events', async () => {
        const { notified } = stubNotification('granted');
        const { handleWsMessage } = useNotifications();
        handleWsMessage({ type: 'SyncPing' });
        await new Promise((r) => setTimeout(r, 10));
        expect(notified.length).toBe(0);
    });

    it('formats singular file count correctly', async () => {
        const { notified } = stubNotification('granted');
        const { handleWsMessage } = useNotifications();
        handleWsMessage({ type: 'ReindexComplete', vault_id: 'v1', file_count: 1, duration_ms: 800 });
        await new Promise((r) => setTimeout(r, 10));

        expect(notified.length).toBeGreaterThan(0);
        expect(notified[0].body).toContain('1 file indexed');
        expect(notified[0].body).not.toContain('files');
    });
});
