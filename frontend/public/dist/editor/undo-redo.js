// Undo/Redo System - Command Pattern Implementation
export class TextChangeCommand {
    constructor(oldContent, newContent) {
        this.oldContent = oldContent;
        this.newContent = newContent;
        this.timestamp = Date.now();
    }
    execute() {
        return this.newContent;
    }
    undo() {
        return this.oldContent;
    }
}
export class UndoRedoManager {
    constructor(initialContent, options) {
        this.undoStack = [];
        this.redoStack = [];
        this.maxStackSize = 100;
        this.debounceTimeout = null;
        this.pendingOldContent = null;
        this.debounceMs = 300;
        this.lastContent = initialContent;
        if (options?.maxStackSize) {
            this.maxStackSize = options.maxStackSize;
        }
        if (options?.debounceMs !== undefined) {
            this.debounceMs = options.debounceMs;
        }
    }
    // Call this when content changes - handles debouncing for smoother UX
    recordChange(newContent) {
        if (newContent === this.lastContent)
            return;
        // Store the old content before any pending changes
        if (this.pendingOldContent === null) {
            this.pendingOldContent = this.lastContent;
        }
        // Clear any existing debounce timer
        if (this.debounceTimeout !== null) {
            clearTimeout(this.debounceTimeout);
        }
        // Set a new debounce timer
        this.debounceTimeout = window.setTimeout(() => {
            this.commitChange(newContent);
        }, this.debounceMs);
        // Update lastContent immediately for tracking
        this.lastContent = newContent;
    }
    // Force commit any pending changes (e.g., before save or undo)
    flushPendingChanges() {
        if (this.debounceTimeout !== null) {
            clearTimeout(this.debounceTimeout);
            this.debounceTimeout = null;
        }
        if (this.pendingOldContent !== null && this.pendingOldContent !== this.lastContent) {
            const command = new TextChangeCommand(this.pendingOldContent, this.lastContent);
            this.pushCommand(command);
        }
        this.pendingOldContent = null;
    }
    commitChange(newContent) {
        if (this.pendingOldContent === null || this.pendingOldContent === newContent) {
            this.pendingOldContent = null;
            return;
        }
        const command = new TextChangeCommand(this.pendingOldContent, newContent);
        this.pushCommand(command);
        this.pendingOldContent = null;
        this.debounceTimeout = null;
    }
    pushCommand(command) {
        this.undoStack.push(command);
        this.redoStack = []; // Clear redo stack on new change
        // Limit stack size
        if (this.undoStack.length > this.maxStackSize) {
            this.undoStack.shift();
        }
    }
    undo() {
        this.flushPendingChanges();
        if (this.undoStack.length === 0)
            return null;
        const command = this.undoStack.pop();
        this.redoStack.push(command);
        const content = command.undo();
        this.lastContent = content;
        return content;
    }
    redo() {
        this.flushPendingChanges();
        if (this.redoStack.length === 0)
            return null;
        const command = this.redoStack.pop();
        this.undoStack.push(command);
        const content = command.execute();
        this.lastContent = content;
        return content;
    }
    canUndo() {
        return this.undoStack.length > 0 || this.pendingOldContent !== null;
    }
    canRedo() {
        return this.redoStack.length > 0;
    }
    // Reset the manager (e.g., after save or reload)
    reset(content) {
        this.undoStack = [];
        this.redoStack = [];
        this.lastContent = content;
        this.pendingOldContent = null;
        if (this.debounceTimeout !== null) {
            clearTimeout(this.debounceTimeout);
            this.debounceTimeout = null;
        }
    }
    // Get current content (for sync purposes)
    getCurrentContent() {
        return this.lastContent;
    }
    // Get stack sizes (for testing/debugging)
    getUndoStackSize() {
        return this.undoStack.length;
    }
    getRedoStackSize() {
        return this.redoStack.length;
    }
}
