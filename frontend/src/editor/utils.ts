// Editor utility functions

export type FileType = 'markdown' | 'image' | 'pdf' | 'text' | 'audio' | 'video' | 'other';

export function getFileType(filePath: string): FileType {
    const ext = filePath.split('.').pop()?.toLowerCase();
    if (!ext) return 'other';

    if (ext === 'md') return 'markdown';
    if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext)) return 'image';
    if (ext === 'pdf') return 'pdf';
    if (['mp3', 'wav', 'ogg'].includes(ext)) return 'audio';
    if (['mp4', 'webm'].includes(ext)) return 'video';
    if (['txt', 'json', 'js', 'ts', 'css', 'html', 'xml'].includes(ext)) return 'text';
    return 'other';
}

export function isImageFile(filePath: string): boolean {
    return getFileType(filePath) === 'image';
}

export function isMarkdownFile(filePath: string): boolean {
    return getFileType(filePath) === 'markdown';
}

export function debounce<T extends (...args: any[]) => any>(
    func: T,
    wait: number
): (...args: Parameters<T>) => void {
    let timeout: ReturnType<typeof setTimeout> | null = null;
    return function (this: any, ...args: Parameters<T>) {
        if (timeout !== null) {
            clearTimeout(timeout);
        }
        timeout = setTimeout(() => func.apply(this, args), wait);
    };
}

// Keyboard shortcut utilities
export interface KeyboardShortcut {
    key: string;
    ctrlKey?: boolean;
    metaKey?: boolean;
    shiftKey?: boolean;
    altKey?: boolean;
}

export function matchesShortcut(event: KeyboardEvent, shortcut: KeyboardShortcut): boolean {
    const ctrlOrMeta = shortcut.ctrlKey || shortcut.metaKey;
    const eventCtrlOrMeta = event.ctrlKey || event.metaKey;

    if (ctrlOrMeta && !eventCtrlOrMeta) return false;
    if (!ctrlOrMeta && eventCtrlOrMeta) return false;
    if (shortcut.shiftKey && !event.shiftKey) return false;
    if (!shortcut.shiftKey && event.shiftKey) return false;
    if (shortcut.altKey && !event.altKey) return false;
    if (!shortcut.altKey && event.altKey) return false;

    return event.key.toLowerCase() === shortcut.key.toLowerCase();
}

// Common keyboard shortcuts
export const SHORTCUTS = {
    UNDO: { key: 'z', ctrlKey: true } as KeyboardShortcut,
    REDO_CTRL_Y: { key: 'y', ctrlKey: true } as KeyboardShortcut,
    REDO_CTRL_SHIFT_Z: { key: 'z', ctrlKey: true, shiftKey: true } as KeyboardShortcut,
    SAVE: { key: 's', ctrlKey: true } as KeyboardShortcut,
    FIND: { key: 'f', ctrlKey: true } as KeyboardShortcut,
    INSERT_LINK: { key: 'k', ctrlKey: true } as KeyboardShortcut,
};

export function isUndoShortcut(event: KeyboardEvent): boolean {
    return matchesShortcut(event, SHORTCUTS.UNDO) && !event.shiftKey;
}

export function isRedoShortcut(event: KeyboardEvent): boolean {
    return matchesShortcut(event, SHORTCUTS.REDO_CTRL_Y) ||
           matchesShortcut(event, SHORTCUTS.REDO_CTRL_SHIFT_Z);
}

export function isSaveShortcut(event: KeyboardEvent): boolean {
    return matchesShortcut(event, SHORTCUTS.SAVE);
}

export function isFindShortcut(event: KeyboardEvent): boolean {
    return matchesShortcut(event, SHORTCUTS.FIND);
}

export function isInsertLinkShortcut(event: KeyboardEvent): boolean {
    return matchesShortcut(event, SHORTCUTS.INSERT_LINK);
}

// Editor mode types
export type EditorMode = 'raw' | 'side-by-side' | 'formatted' | 'rendered';

export function isValidEditorMode(mode: string): mode is EditorMode {
    return ['raw', 'side-by-side', 'formatted', 'rendered'].includes(mode);
}
