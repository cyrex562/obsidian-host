import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { UndoRedoManager, TextChangeCommand } from './undo-redo';

describe('TextChangeCommand', () => {
    it('should store old and new content', () => {
        const command = new TextChangeCommand('old', 'new');
        expect(command.undo()).toBe('old');
        expect(command.execute()).toBe('new');
    });

    it('should have a timestamp', () => {
        const before = Date.now();
        const command = new TextChangeCommand('old', 'new');
        const after = Date.now();

        expect(command.timestamp).toBeGreaterThanOrEqual(before);
        expect(command.timestamp).toBeLessThanOrEqual(after);
    });
});

describe('UndoRedoManager', () => {
    let manager: UndoRedoManager;

    beforeEach(() => {
        vi.useFakeTimers();
        manager = new UndoRedoManager('initial content', { debounceMs: 0 });
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    describe('initialization', () => {
        it('should initialize with content', () => {
            expect(manager.getCurrentContent()).toBe('initial content');
        });

        it('should start with empty undo/redo stacks', () => {
            expect(manager.canUndo()).toBe(false);
            expect(manager.canRedo()).toBe(false);
        });

        it('should respect custom maxStackSize', () => {
            const smallManager = new UndoRedoManager('start', { maxStackSize: 2, debounceMs: 0 });

            smallManager.recordChange('change1');
            vi.advanceTimersByTime(10);
            smallManager.recordChange('change2');
            vi.advanceTimersByTime(10);
            smallManager.recordChange('change3');
            vi.advanceTimersByTime(10);

            // Stack should only have 2 items due to maxStackSize
            expect(smallManager.getUndoStackSize()).toBe(2);
        });
    });

    describe('recordChange', () => {
        it('should track content changes', () => {
            manager.recordChange('new content');
            vi.advanceTimersByTime(10);

            expect(manager.getCurrentContent()).toBe('new content');
            expect(manager.canUndo()).toBe(true);
        });

        it('should ignore duplicate content', () => {
            manager.recordChange('initial content'); // Same as initial
            vi.advanceTimersByTime(10);

            expect(manager.canUndo()).toBe(false);
        });

        it('should debounce rapid changes', () => {
            const debouncedManager = new UndoRedoManager('start', { debounceMs: 300 });

            debouncedManager.recordChange('change1');
            debouncedManager.recordChange('change2');
            debouncedManager.recordChange('change3');

            // Before debounce timeout, nothing committed yet
            expect(debouncedManager.getUndoStackSize()).toBe(0);

            vi.advanceTimersByTime(300);

            // After debounce, only one change should be recorded
            expect(debouncedManager.getUndoStackSize()).toBe(1);
            expect(debouncedManager.getCurrentContent()).toBe('change3');
        });
    });

    describe('undo', () => {
        it('should undo a single change', () => {
            manager.recordChange('modified');
            vi.advanceTimersByTime(10);

            const result = manager.undo();

            expect(result).toBe('initial content');
            expect(manager.getCurrentContent()).toBe('initial content');
        });

        it('should return null when nothing to undo', () => {
            const result = manager.undo();
            expect(result).toBe(null);
        });

        it('should enable redo after undo', () => {
            manager.recordChange('modified');
            vi.advanceTimersByTime(10);

            expect(manager.canRedo()).toBe(false);
            manager.undo();
            expect(manager.canRedo()).toBe(true);
        });

        it('should undo multiple changes in order', () => {
            manager.recordChange('change1');
            vi.advanceTimersByTime(10);
            manager.recordChange('change2');
            vi.advanceTimersByTime(10);
            manager.recordChange('change3');
            vi.advanceTimersByTime(10);

            expect(manager.undo()).toBe('change2');
            expect(manager.undo()).toBe('change1');
            expect(manager.undo()).toBe('initial content');
            expect(manager.undo()).toBe(null);
        });
    });

    describe('redo', () => {
        it('should redo an undone change', () => {
            manager.recordChange('modified');
            vi.advanceTimersByTime(10);
            manager.undo();

            const result = manager.redo();

            expect(result).toBe('modified');
            expect(manager.getCurrentContent()).toBe('modified');
        });

        it('should return null when nothing to redo', () => {
            const result = manager.redo();
            expect(result).toBe(null);
        });

        it('should clear redo stack on new change', () => {
            manager.recordChange('change1');
            vi.advanceTimersByTime(10);
            manager.undo();
            expect(manager.canRedo()).toBe(true);

            manager.recordChange('change2');
            vi.advanceTimersByTime(10);
            expect(manager.canRedo()).toBe(false);
        });

        it('should redo multiple changes in order', () => {
            manager.recordChange('change1');
            vi.advanceTimersByTime(10);
            manager.recordChange('change2');
            vi.advanceTimersByTime(10);

            manager.undo();
            manager.undo();

            expect(manager.redo()).toBe('change1');
            expect(manager.redo()).toBe('change2');
            expect(manager.redo()).toBe(null);
        });
    });

    describe('flushPendingChanges', () => {
        it('should commit pending changes immediately', () => {
            const debouncedManager = new UndoRedoManager('start', { debounceMs: 1000 });

            debouncedManager.recordChange('pending');
            expect(debouncedManager.getUndoStackSize()).toBe(0);

            debouncedManager.flushPendingChanges();
            expect(debouncedManager.getUndoStackSize()).toBe(1);
        });

        it('should be called automatically before undo', () => {
            const debouncedManager = new UndoRedoManager('start', { debounceMs: 1000 });

            debouncedManager.recordChange('pending');
            const result = debouncedManager.undo();

            expect(result).toBe('start');
        });
    });

    describe('reset', () => {
        it('should clear all history', () => {
            manager.recordChange('change1');
            vi.advanceTimersByTime(10);
            manager.recordChange('change2');
            vi.advanceTimersByTime(10);
            manager.undo();

            manager.reset('fresh start');

            expect(manager.getCurrentContent()).toBe('fresh start');
            expect(manager.canUndo()).toBe(false);
            expect(manager.canRedo()).toBe(false);
        });

        it('should clear pending changes', () => {
            const debouncedManager = new UndoRedoManager('start', { debounceMs: 1000 });
            debouncedManager.recordChange('pending');

            debouncedManager.reset('fresh');
            vi.advanceTimersByTime(1000);

            expect(debouncedManager.canUndo()).toBe(false);
        });
    });

    describe('canUndo/canRedo', () => {
        it('canUndo should return true with pending changes', () => {
            const debouncedManager = new UndoRedoManager('start', { debounceMs: 1000 });

            expect(debouncedManager.canUndo()).toBe(false);
            debouncedManager.recordChange('pending');
            expect(debouncedManager.canUndo()).toBe(true);
        });

        it('canRedo should return false initially', () => {
            expect(manager.canRedo()).toBe(false);
        });

        it('canRedo should return true after undo', () => {
            manager.recordChange('change');
            vi.advanceTimersByTime(10);
            manager.undo();

            expect(manager.canRedo()).toBe(true);
        });
    });

    describe('stack size limit', () => {
        it('should remove oldest entries when stack exceeds max size', () => {
            const smallManager = new UndoRedoManager('start', { maxStackSize: 3, debounceMs: 0 });

            for (let i = 1; i <= 5; i++) {
                smallManager.recordChange(`change${i}`);
                vi.advanceTimersByTime(10);
            }

            expect(smallManager.getUndoStackSize()).toBe(3);

            // Should be able to undo to change2 (oldest kept), not change1
            expect(smallManager.undo()).toBe('change4');
            expect(smallManager.undo()).toBe('change3');
            expect(smallManager.undo()).toBe('change2');
            expect(smallManager.undo()).toBe(null);
        });
    });
});
