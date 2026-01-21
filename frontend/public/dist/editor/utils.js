// Editor utility functions
export function getFileType(filePath) {
    const ext = filePath.split('.').pop()?.toLowerCase();
    if (!ext)
        return 'other';
    if (ext === 'md')
        return 'markdown';
    if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext))
        return 'image';
    if (ext === 'pdf')
        return 'pdf';
    if (['mp3', 'wav', 'ogg'].includes(ext))
        return 'audio';
    if (['mp4', 'webm'].includes(ext))
        return 'video';
    if (['txt', 'json', 'js', 'ts', 'css', 'html', 'xml'].includes(ext))
        return 'text';
    return 'other';
}
export function isImageFile(filePath) {
    return getFileType(filePath) === 'image';
}
export function isMarkdownFile(filePath) {
    return getFileType(filePath) === 'markdown';
}
export function debounce(func, wait) {
    let timeout = null;
    return function (...args) {
        if (timeout !== null) {
            clearTimeout(timeout);
        }
        timeout = setTimeout(() => func.apply(this, args), wait);
    };
}
export function matchesShortcut(event, shortcut) {
    const ctrlOrMeta = shortcut.ctrlKey || shortcut.metaKey;
    const eventCtrlOrMeta = event.ctrlKey || event.metaKey;
    if (ctrlOrMeta && !eventCtrlOrMeta)
        return false;
    if (!ctrlOrMeta && eventCtrlOrMeta)
        return false;
    if (shortcut.shiftKey && !event.shiftKey)
        return false;
    if (!shortcut.shiftKey && event.shiftKey)
        return false;
    if (shortcut.altKey && !event.altKey)
        return false;
    if (!shortcut.altKey && event.altKey)
        return false;
    return event.key.toLowerCase() === shortcut.key.toLowerCase();
}
// Common keyboard shortcuts
export const SHORTCUTS = {
    UNDO: { key: 'z', ctrlKey: true },
    REDO_CTRL_Y: { key: 'y', ctrlKey: true },
    REDO_CTRL_SHIFT_Z: { key: 'z', ctrlKey: true, shiftKey: true },
    SAVE: { key: 's', ctrlKey: true },
    FIND: { key: 'f', ctrlKey: true },
    INSERT_LINK: { key: 'k', ctrlKey: true },
};
export function isUndoShortcut(event) {
    return matchesShortcut(event, SHORTCUTS.UNDO) && !event.shiftKey;
}
export function isRedoShortcut(event) {
    return matchesShortcut(event, SHORTCUTS.REDO_CTRL_Y) ||
        matchesShortcut(event, SHORTCUTS.REDO_CTRL_SHIFT_Z);
}
export function isSaveShortcut(event) {
    return matchesShortcut(event, SHORTCUTS.SAVE);
}
export function isFindShortcut(event) {
    return matchesShortcut(event, SHORTCUTS.FIND);
}
export function isInsertLinkShortcut(event) {
    return matchesShortcut(event, SHORTCUTS.INSERT_LINK);
}
export function isValidEditorMode(mode) {
    return ['raw', 'side-by-side', 'formatted', 'rendered'].includes(mode);
}
