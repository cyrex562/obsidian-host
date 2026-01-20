import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { UndoRedoManager } from './undo-redo';
import { isUndoShortcut, isRedoShortcut, isSaveShortcut } from './utils';

/**
 * Editor Interaction Tests
 *
 * These tests verify the core editor interactions:
 * - Text input and content tracking
 * - Dirty state management
 * - Undo/Redo integration with editor
 * - Mode switching behavior
 * - Keyboard shortcut handling
 */

describe('Editor Interactions', () => {
    let textarea: HTMLTextAreaElement;
    let undoManager: UndoRedoManager;
    let isDirty: boolean;
    let content: string;

    beforeEach(() => {
        vi.useFakeTimers();

        // Set up DOM elements
        textarea = document.createElement('textarea');
        textarea.id = 'editor-textarea';
        document.body.appendChild(textarea);

        // Initialize state
        content = 'Initial content';
        isDirty = false;
        undoManager = new UndoRedoManager(content, { debounceMs: 0 });
        textarea.value = content;
    });

    afterEach(() => {
        vi.useRealTimers();
        document.body.innerHTML = '';
    });

    describe('Raw Editor Input Handling', () => {
        function setupInputHandler() {
            textarea.addEventListener('input', () => {
                const newContent = textarea.value;
                undoManager.recordChange(newContent);
                content = newContent;
                isDirty = true;
            });
        }

        it('should update content on text input', () => {
            setupInputHandler();

            textarea.value = 'New content';
            textarea.dispatchEvent(new Event('input'));

            expect(content).toBe('New content');
        });

        it('should mark as dirty on change', () => {
            setupInputHandler();

            expect(isDirty).toBe(false);

            textarea.value = 'Modified';
            textarea.dispatchEvent(new Event('input'));

            expect(isDirty).toBe(true);
        });

        it('should record changes in undo manager', () => {
            setupInputHandler();

            textarea.value = 'First edit';
            textarea.dispatchEvent(new Event('input'));
            vi.advanceTimersByTime(10);

            expect(undoManager.canUndo()).toBe(true);
        });

        it('should handle multiple rapid inputs', () => {
            setupInputHandler();

            textarea.value = 'A';
            textarea.dispatchEvent(new Event('input'));
            textarea.value = 'AB';
            textarea.dispatchEvent(new Event('input'));
            textarea.value = 'ABC';
            textarea.dispatchEvent(new Event('input'));
            vi.advanceTimersByTime(10);

            expect(content).toBe('ABC');
            expect(isDirty).toBe(true);
        });

        it('should handle paste events', () => {
            setupInputHandler();

            // Simulate paste by directly setting value
            textarea.value = 'Pasted text from clipboard';
            textarea.dispatchEvent(new Event('input'));

            expect(content).toBe('Pasted text from clipboard');
            expect(isDirty).toBe(true);
        });

        it('should handle delete/backspace', () => {
            setupInputHandler();

            textarea.value = 'Initial conten'; // simulating backspace
            textarea.dispatchEvent(new Event('input'));

            expect(content).toBe('Initial conten');
            expect(isDirty).toBe(true);
        });

        it('should handle clearing all content', () => {
            setupInputHandler();

            textarea.value = '';
            textarea.dispatchEvent(new Event('input'));

            expect(content).toBe('');
            expect(isDirty).toBe(true);
        });
    });

    describe('Undo/Redo Integration', () => {
        function setupWithUndoRedo() {
            textarea.addEventListener('input', () => {
                const newContent = textarea.value;
                undoManager.recordChange(newContent);
                content = newContent;
                isDirty = true;
            });
        }

        function performUndo() {
            const previousContent = undoManager.undo();
            if (previousContent !== null) {
                content = previousContent;
                textarea.value = previousContent;
            }
            return previousContent;
        }

        function performRedo() {
            const nextContent = undoManager.redo();
            if (nextContent !== null) {
                content = nextContent;
                textarea.value = nextContent;
            }
            return nextContent;
        }

        it('should undo text input', () => {
            setupWithUndoRedo();

            textarea.value = 'Modified text';
            textarea.dispatchEvent(new Event('input'));
            vi.advanceTimersByTime(10);

            performUndo();

            expect(textarea.value).toBe('Initial content');
            expect(content).toBe('Initial content');
        });

        it('should redo undone changes', () => {
            setupWithUndoRedo();

            textarea.value = 'Modified text';
            textarea.dispatchEvent(new Event('input'));
            vi.advanceTimersByTime(10);

            performUndo();
            performRedo();

            expect(textarea.value).toBe('Modified text');
        });

        it('should handle multiple undo/redo cycles', () => {
            setupWithUndoRedo();

            // Make several changes
            textarea.value = 'Change 1';
            textarea.dispatchEvent(new Event('input'));
            vi.advanceTimersByTime(10);

            textarea.value = 'Change 2';
            textarea.dispatchEvent(new Event('input'));
            vi.advanceTimersByTime(10);

            textarea.value = 'Change 3';
            textarea.dispatchEvent(new Event('input'));
            vi.advanceTimersByTime(10);

            // Undo all
            expect(performUndo()).toBe('Change 2');
            expect(performUndo()).toBe('Change 1');
            expect(performUndo()).toBe('Initial content');

            // Redo all
            expect(performRedo()).toBe('Change 1');
            expect(performRedo()).toBe('Change 2');
            expect(performRedo()).toBe('Change 3');
        });

        it('should clear redo stack on new input after undo', () => {
            setupWithUndoRedo();

            textarea.value = 'First change';
            textarea.dispatchEvent(new Event('input'));
            vi.advanceTimersByTime(10);

            performUndo();
            expect(undoManager.canRedo()).toBe(true);

            // New input should clear redo
            textarea.value = 'Different change';
            textarea.dispatchEvent(new Event('input'));
            vi.advanceTimersByTime(10);

            expect(undoManager.canRedo()).toBe(false);
        });
    });

    describe('Keyboard Shortcut Handling', () => {
        function createKeyboardEvent(key: string, modifiers: { ctrl?: boolean; shift?: boolean; meta?: boolean } = {}): KeyboardEvent {
            return new KeyboardEvent('keydown', {
                key,
                ctrlKey: modifiers.ctrl || false,
                shiftKey: modifiers.shift || false,
                metaKey: modifiers.meta || false,
                bubbles: true,
            });
        }

        it('should detect Ctrl+Z as undo shortcut', () => {
            const event = createKeyboardEvent('z', { ctrl: true });
            expect(isUndoShortcut(event)).toBe(true);
        });

        it('should detect Cmd+Z as undo shortcut (Mac)', () => {
            const event = createKeyboardEvent('z', { meta: true });
            expect(isUndoShortcut(event)).toBe(true);
        });

        it('should detect Ctrl+Y as redo shortcut', () => {
            const event = createKeyboardEvent('y', { ctrl: true });
            expect(isRedoShortcut(event)).toBe(true);
        });

        it('should detect Ctrl+Shift+Z as redo shortcut', () => {
            const event = createKeyboardEvent('z', { ctrl: true, shift: true });
            expect(isRedoShortcut(event)).toBe(true);
        });

        it('should detect Ctrl+S as save shortcut', () => {
            const event = createKeyboardEvent('s', { ctrl: true });
            expect(isSaveShortcut(event)).toBe(true);
        });

        it('should integrate keyboard shortcuts with undo/redo', () => {
            let lastAction = '';

            textarea.addEventListener('input', () => {
                undoManager.recordChange(textarea.value);
                content = textarea.value;
            });

            document.addEventListener('keydown', (e) => {
                if (isUndoShortcut(e)) {
                    e.preventDefault();
                    const prev = undoManager.undo();
                    if (prev !== null) {
                        textarea.value = prev;
                        content = prev;
                        lastAction = 'undo';
                    }
                } else if (isRedoShortcut(e)) {
                    e.preventDefault();
                    const next = undoManager.redo();
                    if (next !== null) {
                        textarea.value = next;
                        content = next;
                        lastAction = 'redo';
                    }
                }
            });

            // Make a change
            textarea.value = 'Changed';
            textarea.dispatchEvent(new Event('input'));
            vi.advanceTimersByTime(10);

            // Trigger Ctrl+Z
            document.dispatchEvent(createKeyboardEvent('z', { ctrl: true }));

            expect(lastAction).toBe('undo');
            expect(textarea.value).toBe('Initial content');

            // Trigger Ctrl+Y
            document.dispatchEvent(createKeyboardEvent('y', { ctrl: true }));

            expect(lastAction).toBe('redo');
            expect(textarea.value).toBe('Changed');
        });
    });

    describe('Editor Mode Switching', () => {
        let editorMode: 'raw' | 'side-by-side' | 'formatted' | 'rendered';
        let modeButtons: HTMLButtonElement[];

        beforeEach(() => {
            editorMode = 'raw';

            // Create mode selector buttons
            const modes = ['raw', 'side-by-side', 'formatted', 'rendered'] as const;
            modeButtons = modes.map(mode => {
                const btn = document.createElement('button');
                btn.className = 'mode-btn';
                btn.dataset.mode = mode;
                btn.textContent = mode;
                if (mode === 'raw') btn.classList.add('active');
                document.body.appendChild(btn);
                return btn;
            });
        });

        function switchMode(newMode: typeof editorMode) {
            editorMode = newMode;
            modeButtons.forEach(btn => {
                btn.classList.toggle('active', btn.dataset.mode === newMode);
            });
        }

        it('should preserve content when switching modes', () => {
            const testContent = 'Test content for mode switching';
            content = testContent;
            textarea.value = testContent;

            switchMode('formatted');
            expect(content).toBe(testContent);

            switchMode('rendered');
            expect(content).toBe(testContent);

            switchMode('raw');
            expect(content).toBe(testContent);
        });

        it('should update active button class on mode switch', () => {
            expect(modeButtons[0].classList.contains('active')).toBe(true); // raw
            expect(modeButtons[1].classList.contains('active')).toBe(false); // side-by-side

            switchMode('side-by-side');

            expect(modeButtons[0].classList.contains('active')).toBe(false);
            expect(modeButtons[1].classList.contains('active')).toBe(true);
        });

        it('should preserve dirty state across mode switches', () => {
            isDirty = true;
            switchMode('formatted');
            expect(isDirty).toBe(true);
        });

        it('should preserve undo history across mode switches', () => {
            textarea.addEventListener('input', () => {
                undoManager.recordChange(textarea.value);
                content = textarea.value;
            });

            textarea.value = 'Change before mode switch';
            textarea.dispatchEvent(new Event('input'));
            vi.advanceTimersByTime(10);

            switchMode('formatted');

            expect(undoManager.canUndo()).toBe(true);
            expect(undoManager.undo()).toBe('Initial content');
        });
    });

    describe('Auto-save Behavior', () => {
        let saveCount: number;
        let lastSavedContent: string;

        beforeEach(() => {
            saveCount = 0;
            lastSavedContent = '';
        });

        function simulateAutoSave(interval: number) {
            return setInterval(() => {
                if (isDirty) {
                    lastSavedContent = content;
                    isDirty = false;
                    saveCount++;
                }
            }, interval);
        }

        it('should auto-save dirty content', () => {
            const autoSaveInterval = simulateAutoSave(1000);

            textarea.value = 'Changed content';
            content = textarea.value;
            isDirty = true;

            vi.advanceTimersByTime(1000);

            expect(saveCount).toBe(1);
            expect(lastSavedContent).toBe('Changed content');
            expect(isDirty).toBe(false);

            clearInterval(autoSaveInterval);
        });

        it('should not save when not dirty', () => {
            const autoSaveInterval = simulateAutoSave(1000);

            vi.advanceTimersByTime(3000);

            expect(saveCount).toBe(0);

            clearInterval(autoSaveInterval);
        });

        it('should batch rapid changes into single save', () => {
            const autoSaveInterval = simulateAutoSave(1000);

            // Make several rapid changes
            for (let i = 0; i < 5; i++) {
                textarea.value = `Change ${i}`;
                content = textarea.value;
                isDirty = true;
                vi.advanceTimersByTime(100);
            }

            vi.advanceTimersByTime(1000);

            expect(saveCount).toBe(1);
            expect(lastSavedContent).toBe('Change 4');

            clearInterval(autoSaveInterval);
        });
    });

    describe('Tab State Management', () => {
        interface TabState {
            id: string;
            content: string;
            isDirty: boolean;
            undoManager: UndoRedoManager;
        }

        let tabs: Map<string, TabState>;
        let activeTabId: string;

        beforeEach(() => {
            tabs = new Map();
            activeTabId = 'tab1';

            tabs.set('tab1', {
                id: 'tab1',
                content: 'Tab 1 content',
                isDirty: false,
                undoManager: new UndoRedoManager('Tab 1 content', { debounceMs: 0 }),
            });

            tabs.set('tab2', {
                id: 'tab2',
                content: 'Tab 2 content',
                isDirty: false,
                undoManager: new UndoRedoManager('Tab 2 content', { debounceMs: 0 }),
            });
        });

        function switchTab(tabId: string) {
            const tab = tabs.get(tabId);
            if (tab) {
                activeTabId = tabId;
                textarea.value = tab.content;
            }
        }

        function getActiveTab() {
            return tabs.get(activeTabId);
        }

        it('should maintain separate undo history per tab', () => {
            const tab1 = tabs.get('tab1')!;
            const tab2 = tabs.get('tab2')!;

            // Edit tab 1
            tab1.undoManager.recordChange('Tab 1 edited');
            tab1.content = 'Tab 1 edited';
            vi.advanceTimersByTime(10);

            // Edit tab 2
            tab2.undoManager.recordChange('Tab 2 edited');
            tab2.content = 'Tab 2 edited';
            vi.advanceTimersByTime(10);

            // Undo on tab 1 should not affect tab 2
            expect(tab1.undoManager.undo()).toBe('Tab 1 content');
            expect(tab2.undoManager.getCurrentContent()).toBe('Tab 2 edited');
        });

        it('should preserve content when switching tabs', () => {
            textarea.value = 'Modified tab 1';
            const tab1 = getActiveTab()!;
            tab1.content = textarea.value;

            switchTab('tab2');
            expect(textarea.value).toBe('Tab 2 content');

            switchTab('tab1');
            expect(tabs.get('tab1')!.content).toBe('Modified tab 1');
        });

        it('should track dirty state per tab', () => {
            const tab1 = tabs.get('tab1')!;
            const tab2 = tabs.get('tab2')!;

            tab1.isDirty = true;

            expect(tab1.isDirty).toBe(true);
            expect(tab2.isDirty).toBe(false);
        });
    });
});
