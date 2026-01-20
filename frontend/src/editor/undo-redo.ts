// Undo/Redo System - Command Pattern Implementation

export interface EditCommand {
    execute(): string;  // Returns new content
    undo(): string;     // Returns previous content
    timestamp: number;
}

export class TextChangeCommand implements EditCommand {
    timestamp: number;

    constructor(
        private oldContent: string,
        private newContent: string
    ) {
        this.timestamp = Date.now();
    }

    execute(): string {
        return this.newContent;
    }

    undo(): string {
        return this.oldContent;
    }
}

export class UndoRedoManager {
    private undoStack: EditCommand[] = [];
    private redoStack: EditCommand[] = [];
    private maxStackSize: number = 100;
    private lastContent: string;
    private debounceTimeout: number | null = null;
    private pendingOldContent: string | null = null;
    private debounceMs: number = 300;

    constructor(initialContent: string, options?: { maxStackSize?: number; debounceMs?: number }) {
        this.lastContent = initialContent;
        if (options?.maxStackSize) {
            this.maxStackSize = options.maxStackSize;
        }
        if (options?.debounceMs !== undefined) {
            this.debounceMs = options.debounceMs;
        }
    }

    // Call this when content changes - handles debouncing for smoother UX
    recordChange(newContent: string): void {
        if (newContent === this.lastContent) return;

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
    flushPendingChanges(): void {
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

    private commitChange(newContent: string): void {
        if (this.pendingOldContent === null || this.pendingOldContent === newContent) {
            this.pendingOldContent = null;
            return;
        }

        const command = new TextChangeCommand(this.pendingOldContent, newContent);
        this.pushCommand(command);
        this.pendingOldContent = null;
        this.debounceTimeout = null;
    }

    private pushCommand(command: EditCommand): void {
        this.undoStack.push(command);
        this.redoStack = []; // Clear redo stack on new change

        // Limit stack size
        if (this.undoStack.length > this.maxStackSize) {
            this.undoStack.shift();
        }
    }

    undo(): string | null {
        this.flushPendingChanges();

        if (this.undoStack.length === 0) return null;

        const command = this.undoStack.pop()!;
        this.redoStack.push(command);

        const content = command.undo();
        this.lastContent = content;
        return content;
    }

    redo(): string | null {
        this.flushPendingChanges();

        if (this.redoStack.length === 0) return null;

        const command = this.redoStack.pop()!;
        this.undoStack.push(command);

        const content = command.execute();
        this.lastContent = content;
        return content;
    }

    canUndo(): boolean {
        return this.undoStack.length > 0 || this.pendingOldContent !== null;
    }

    canRedo(): boolean {
        return this.redoStack.length > 0;
    }

    // Reset the manager (e.g., after save or reload)
    reset(content: string): void {
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
    getCurrentContent(): string {
        return this.lastContent;
    }

    // Get stack sizes (for testing/debugging)
    getUndoStackSize(): number {
        return this.undoStack.length;
    }

    getRedoStackSize(): number {
        return this.redoStack.length;
    }
}
