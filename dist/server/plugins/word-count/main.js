/**
 * Word Count Plugin
 * Displays statistics about the current document
 */

class WordCountPlugin {
    constructor(api) {
        this.api = api;
        this.config = {};
        this.statusBarItem = null;
    }

    async onLoad(ctx) {
        console.log('Word Count plugin loaded', ctx);

        // Load configuration
        const savedConfig = await this.api.storage_get('config');
        this.config = savedConfig || this.getDefaultConfig();

        // Add status bar item
        this.statusBarItem = this.api.addStatusBarItem();
        this.updateStatusBar('');

        console.log('Word Count plugin initialized');
    }

    async onFileOpen(ctx, filePath) {
        // Read file content and update stats
        try {
            const content = await this.api.read_file(ctx.vault_id, filePath);
            this.updateStats(content);
        } catch (error) {
            console.error('Failed to read file for word count:', error);
        }
    }

    async onEditorChange(ctx, content) {
        this.updateStats(content);
    }

    async onUnload() {
        if (this.statusBarItem) {
            this.statusBarItem.remove();
        }
        console.log('Word Count plugin unloaded');
    }

    getDefaultConfig() {
        return {
            show_word_count: true,
            show_char_count: true,
            show_reading_time: true,
            words_per_minute: 200
        };
    }

    updateStats(content) {
        const stats = this.calculateStats(content);
        this.updateStatusBar(this.formatStats(stats));
    }

    calculateStats(content) {
        // Remove frontmatter
        const contentWithoutFrontmatter = content.replace(/^---\n[\s\S]*?\n---\n/, '');

        // Remove code blocks
        const contentWithoutCode = contentWithoutFrontmatter.replace(/```[\s\S]*?```/g, '');

        // Count words
        const words = contentWithoutCode
            .trim()
            .split(/\s+/)
            .filter(word => word.length > 0);
        const wordCount = words.length;

        // Count characters (excluding spaces)
        const charCount = contentWithoutCode.replace(/\s/g, '').length;

        // Calculate reading time
        const readingTimeMinutes = Math.ceil(wordCount / this.config.words_per_minute);

        return {
            words: wordCount,
            characters: charCount,
            readingTime: readingTimeMinutes
        };
    }

    formatStats(stats) {
        const parts = [];

        if (this.config.show_word_count) {
            parts.push(`${stats.words} words`);
        }

        if (this.config.show_char_count) {
            parts.push(`${stats.characters} chars`);
        }

        if (this.config.show_reading_time && stats.words > 0) {
            parts.push(`${stats.readingTime} min read`);
        }

        return parts.join(' • ');
    }

    updateStatusBar(text) {
        if (this.statusBarItem) {
            this.statusBarItem.setText(text || 'No content');
        }
    }
}

export default WordCountPlugin;
