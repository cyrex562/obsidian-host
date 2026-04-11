/**
 * Backlinks Plugin
 * Shows all notes that link to the current note
 */

class BacklinksPlugin {
    constructor(api) {
        this.api = api;
        this.config = {};
        this.currentFile = null;
        this.backlinksCache = new Map();
    }

    async onLoad(ctx) {
        console.log('Backlinks plugin loaded', ctx);

        // Load configuration
        const savedConfig = await this.api.storage_get('config');
        this.config = savedConfig || this.getDefaultConfig();

        // Build initial backlinks index
        await this.rebuildIndex();

        console.log('Backlinks plugin initialized');
    }

    async onFileOpen(ctx, filePath) {
        this.currentFile = filePath;
        await this.showBacklinks(filePath);
    }

    async onFileSave(ctx, filePath) {
        // Rebuild index when files are saved
        await this.updateBacklinksForFile(filePath);
    }

    async onUnload() {
        console.log('Backlinks plugin unloaded');
    }

    getDefaultConfig() {
        return {
            show_unlinked_mentions: true,
            case_sensitive: false
        };
    }

    async rebuildIndex() {
        console.log('Building backlinks index...');
        this.backlinksCache.clear();

        try {
            const files = await this.api.list_files(this.api.getContext().vault_id, '*.md');

            for (const file of files) {
                await this.updateBacklinksForFile(file);
            }

            console.log(`Backlinks index built: ${this.backlinksCache.size} files indexed`);
        } catch (error) {
            console.error('Failed to build backlinks index:', error);
        }
    }

    async updateBacklinksForFile(filePath) {
        try {
            const content = await this.api.read_file(this.api.getContext().vault_id, filePath);
            const links = this.extractLinks(content);

            // Store outgoing links for this file
            this.backlinksCache.set(filePath, links);
        } catch (error) {
            console.error(`Failed to update backlinks for ${filePath}:`, error);
        }
    }

    extractLinks(content) {
        const links = [];

        // Extract wiki links [[Note Name]]
        const wikiLinkRegex = /\[\[([^\]]+)\]\]/g;
        let match;

        while ((match = wikiLinkRegex.exec(content)) !== null) {
            let linkText = match[1];

            // Handle [[Note|Alias]] format
            if (linkText.includes('|')) {
                linkText = linkText.split('|')[0];
            }

            links.push(linkText.trim());
        }

        return links;
    }

    async showBacklinks(filePath) {
        const backlinks = this.findBacklinks(filePath);
        const unlinkedMentions = this.config.show_unlinked_mentions
            ? await this.findUnlinkedMentions(filePath)
            : [];

        // Display backlinks in UI
        this.displayBacklinks(backlinks, unlinkedMentions);
    }

    findBacklinks(targetFile) {
        const backlinks = [];
        const targetName = this.getFileNameWithoutExtension(targetFile);

        for (const [sourceFile, links] of this.backlinksCache.entries()) {
            if (sourceFile === targetFile) continue;

            for (const link of links) {
                const linkName = this.getFileNameWithoutExtension(link);
                if (linkName === targetName) {
                    backlinks.push({
                        file: sourceFile,
                        type: 'link'
                    });
                    break;
                }
            }
        }

        return backlinks;
    }

    async findUnlinkedMentions(targetFile) {
        const mentions = [];
        const targetName = this.getFileNameWithoutExtension(targetFile);

        try {
            const files = await this.api.list_files(this.api.getContext().vault_id, '*.md');

            for (const file of files) {
                if (file === targetFile) continue;

                try {
                    const content = await this.api.read_file(this.api.getContext().vault_id, file);

                    // Remove wiki links to avoid double counting
                    const contentWithoutLinks = content.replace(/\[\[[^\]]+\]\]/g, '');

                    // Search for mentions
                    const searchText = this.config.case_sensitive
                        ? contentWithoutLinks
                        : contentWithoutLinks.toLowerCase();
                    const searchTerm = this.config.case_sensitive
                        ? targetName
                        : targetName.toLowerCase();

                    if (searchText.includes(searchTerm)) {
                        mentions.push({
                            file: file,
                            type: 'mention'
                        });
                    }
                } catch (error) {
                    // Skip files that can't be read
                }
            }
        } catch (error) {
            console.error('Failed to find unlinked mentions:', error);
        }

        return mentions;
    }

    displayBacklinks(backlinks, unlinkedMentions) {
        console.log('Backlinks:', backlinks);
        console.log('Unlinked mentions:', unlinkedMentions);

        // TODO: Update UI panel with backlinks
        // For now, just log to console
        const total = backlinks.length + unlinkedMentions.length;
        if (total > 0) {
            this.api.show_notice(`Found ${backlinks.length} backlinks and ${unlinkedMentions.length} mentions`);
        }
    }

    getFileNameWithoutExtension(filePath) {
        const fileName = filePath.split('/').pop() || filePath;
        return fileName.replace(/\.md$/, '');
    }
}

export default BacklinksPlugin;
