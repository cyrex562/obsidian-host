import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
    getFileType,
    isImageFile,
    isMarkdownFile,
    debounce,
    matchesShortcut,
    isUndoShortcut,
    isRedoShortcut,
    isSaveShortcut,
    isFindShortcut,
    isInsertLinkShortcut,
    isValidEditorMode,
    SHORTCUTS,
} from './utils';

describe('File type detection', () => {
    describe('getFileType', () => {
        it('should detect markdown files', () => {
            expect(getFileType('note.md')).toBe('markdown');
            expect(getFileType('path/to/note.md')).toBe('markdown');
            expect(getFileType('NOTE.MD')).toBe('markdown');
        });

        it('should detect image files', () => {
            expect(getFileType('image.png')).toBe('image');
            expect(getFileType('photo.jpg')).toBe('image');
            expect(getFileType('photo.jpeg')).toBe('image');
            expect(getFileType('animation.gif')).toBe('image');
            expect(getFileType('icon.svg')).toBe('image');
            expect(getFileType('image.webp')).toBe('image');
        });

        it('should detect PDF files', () => {
            expect(getFileType('document.pdf')).toBe('pdf');
            expect(getFileType('DOCUMENT.PDF')).toBe('pdf');
        });

        it('should detect audio files', () => {
            expect(getFileType('song.mp3')).toBe('audio');
            expect(getFileType('sound.wav')).toBe('audio');
            expect(getFileType('audio.ogg')).toBe('audio');
        });

        it('should detect video files', () => {
            expect(getFileType('video.mp4')).toBe('video');
            expect(getFileType('video.webm')).toBe('video');
        });

        it('should detect text files', () => {
            expect(getFileType('readme.txt')).toBe('text');
            expect(getFileType('data.json')).toBe('text');
            expect(getFileType('script.js')).toBe('text');
            expect(getFileType('code.ts')).toBe('text');
            expect(getFileType('style.css')).toBe('text');
            expect(getFileType('page.html')).toBe('text');
            expect(getFileType('config.xml')).toBe('text');
        });

        it('should return other for unknown extensions', () => {
            expect(getFileType('file.xyz')).toBe('other');
            expect(getFileType('binary.exe')).toBe('other');
        });

        it('should return other for files without extension', () => {
            expect(getFileType('README')).toBe('other');
            expect(getFileType('Makefile')).toBe('other');
        });
    });

    describe('isImageFile', () => {
        it('should return true for image files', () => {
            expect(isImageFile('photo.jpg')).toBe(true);
            expect(isImageFile('icon.png')).toBe(true);
        });

        it('should return false for non-image files', () => {
            expect(isImageFile('note.md')).toBe(false);
            expect(isImageFile('document.pdf')).toBe(false);
        });
    });

    describe('isMarkdownFile', () => {
        it('should return true for markdown files', () => {
            expect(isMarkdownFile('note.md')).toBe(true);
        });

        it('should return false for non-markdown files', () => {
            expect(isMarkdownFile('note.txt')).toBe(false);
            expect(isMarkdownFile('image.png')).toBe(false);
        });
    });
});

describe('debounce', () => {
    beforeEach(() => {
        vi.useFakeTimers();
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    it('should delay function execution', () => {
        const fn = vi.fn();
        const debounced = debounce(fn, 100);

        debounced();
        expect(fn).not.toHaveBeenCalled();

        vi.advanceTimersByTime(100);
        expect(fn).toHaveBeenCalledTimes(1);
    });

    it('should reset delay on subsequent calls', () => {
        const fn = vi.fn();
        const debounced = debounce(fn, 100);

        debounced();
        vi.advanceTimersByTime(50);
        debounced();
        vi.advanceTimersByTime(50);
        expect(fn).not.toHaveBeenCalled();

        vi.advanceTimersByTime(50);
        expect(fn).toHaveBeenCalledTimes(1);
    });

    it('should pass arguments to the function', () => {
        const fn = vi.fn();
        const debounced = debounce(fn, 100);

        debounced('arg1', 'arg2');
        vi.advanceTimersByTime(100);

        expect(fn).toHaveBeenCalledWith('arg1', 'arg2');
    });

    it('should use latest arguments when debounced', () => {
        const fn = vi.fn();
        const debounced = debounce(fn, 100);

        debounced('first');
        debounced('second');
        debounced('third');
        vi.advanceTimersByTime(100);

        expect(fn).toHaveBeenCalledTimes(1);
        expect(fn).toHaveBeenCalledWith('third');
    });
});

describe('Keyboard shortcuts', () => {
    function createKeyboardEvent(options: Partial<KeyboardEvent>): KeyboardEvent {
        return {
            key: 'a',
            ctrlKey: false,
            metaKey: false,
            shiftKey: false,
            altKey: false,
            ...options,
        } as KeyboardEvent;
    }

    describe('matchesShortcut', () => {
        it('should match simple key', () => {
            const event = createKeyboardEvent({ key: 'a' });
            expect(matchesShortcut(event, { key: 'a' })).toBe(true);
            expect(matchesShortcut(event, { key: 'b' })).toBe(false);
        });

        it('should be case insensitive', () => {
            const event = createKeyboardEvent({ key: 'A' });
            expect(matchesShortcut(event, { key: 'a' })).toBe(true);
        });

        it('should match Ctrl modifier', () => {
            const event = createKeyboardEvent({ key: 'z', ctrlKey: true });
            expect(matchesShortcut(event, { key: 'z', ctrlKey: true })).toBe(true);
            expect(matchesShortcut(event, { key: 'z' })).toBe(false);
        });

        it('should match Meta modifier (Cmd on Mac)', () => {
            const event = createKeyboardEvent({ key: 'z', metaKey: true });
            expect(matchesShortcut(event, { key: 'z', metaKey: true })).toBe(true);
        });

        it('should treat Ctrl and Meta as equivalent', () => {
            const ctrlEvent = createKeyboardEvent({ key: 'z', ctrlKey: true });
            const metaEvent = createKeyboardEvent({ key: 'z', metaKey: true });

            expect(matchesShortcut(ctrlEvent, { key: 'z', ctrlKey: true })).toBe(true);
            expect(matchesShortcut(metaEvent, { key: 'z', ctrlKey: true })).toBe(true);
        });

        it('should match Shift modifier', () => {
            const event = createKeyboardEvent({ key: 'z', ctrlKey: true, shiftKey: true });
            expect(matchesShortcut(event, { key: 'z', ctrlKey: true, shiftKey: true })).toBe(true);
            expect(matchesShortcut(event, { key: 'z', ctrlKey: true })).toBe(false);
        });

        it('should match Alt modifier', () => {
            const event = createKeyboardEvent({ key: 'a', altKey: true });
            expect(matchesShortcut(event, { key: 'a', altKey: true })).toBe(true);
            expect(matchesShortcut(event, { key: 'a' })).toBe(false);
        });
    });

    describe('isUndoShortcut', () => {
        it('should match Ctrl+Z', () => {
            const event = createKeyboardEvent({ key: 'z', ctrlKey: true });
            expect(isUndoShortcut(event)).toBe(true);
        });

        it('should match Cmd+Z', () => {
            const event = createKeyboardEvent({ key: 'z', metaKey: true });
            expect(isUndoShortcut(event)).toBe(true);
        });

        it('should not match Ctrl+Shift+Z (that is redo)', () => {
            const event = createKeyboardEvent({ key: 'z', ctrlKey: true, shiftKey: true });
            expect(isUndoShortcut(event)).toBe(false);
        });

        it('should not match plain Z', () => {
            const event = createKeyboardEvent({ key: 'z' });
            expect(isUndoShortcut(event)).toBe(false);
        });
    });

    describe('isRedoShortcut', () => {
        it('should match Ctrl+Y', () => {
            const event = createKeyboardEvent({ key: 'y', ctrlKey: true });
            expect(isRedoShortcut(event)).toBe(true);
        });

        it('should match Ctrl+Shift+Z', () => {
            const event = createKeyboardEvent({ key: 'z', ctrlKey: true, shiftKey: true });
            expect(isRedoShortcut(event)).toBe(true);
        });

        it('should match Cmd+Shift+Z', () => {
            const event = createKeyboardEvent({ key: 'z', metaKey: true, shiftKey: true });
            expect(isRedoShortcut(event)).toBe(true);
        });

        it('should not match Ctrl+Z (that is undo)', () => {
            const event = createKeyboardEvent({ key: 'z', ctrlKey: true });
            expect(isRedoShortcut(event)).toBe(false);
        });
    });

    describe('isSaveShortcut', () => {
        it('should match Ctrl+S', () => {
            const event = createKeyboardEvent({ key: 's', ctrlKey: true });
            expect(isSaveShortcut(event)).toBe(true);
        });

        it('should match Cmd+S', () => {
            const event = createKeyboardEvent({ key: 's', metaKey: true });
            expect(isSaveShortcut(event)).toBe(true);
        });

        it('should not match plain S', () => {
            const event = createKeyboardEvent({ key: 's' });
            expect(isSaveShortcut(event)).toBe(false);
        });
    });

    describe('isFindShortcut', () => {
        it('should match Ctrl+F', () => {
            const event = createKeyboardEvent({ key: 'f', ctrlKey: true });
            expect(isFindShortcut(event)).toBe(true);
        });

        it('should match Cmd+F', () => {
            const event = createKeyboardEvent({ key: 'f', metaKey: true });
            expect(isFindShortcut(event)).toBe(true);
        });
    });

    describe('isInsertLinkShortcut', () => {
        it('should match Ctrl+K', () => {
            const event = createKeyboardEvent({ key: 'k', ctrlKey: true });
            expect(isInsertLinkShortcut(event)).toBe(true);
        });

        it('should match Cmd+K', () => {
            const event = createKeyboardEvent({ key: 'k', metaKey: true });
            expect(isInsertLinkShortcut(event)).toBe(true);
        });
    });
});

describe('Editor mode validation', () => {
    describe('isValidEditorMode', () => {
        it('should return true for valid modes', () => {
            expect(isValidEditorMode('raw')).toBe(true);
            expect(isValidEditorMode('side-by-side')).toBe(true);
            expect(isValidEditorMode('formatted')).toBe(true);
            expect(isValidEditorMode('rendered')).toBe(true);
        });

        it('should return false for invalid modes', () => {
            expect(isValidEditorMode('invalid')).toBe(false);
            expect(isValidEditorMode('')).toBe(false);
            expect(isValidEditorMode('RAW')).toBe(false);
        });
    });
});
