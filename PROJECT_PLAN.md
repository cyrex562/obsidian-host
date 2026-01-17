# Obsidian Web UI - Project Plan

## Feature 1: Project Setup & Infrastructure

### Task 1.1: Initialize Rust backend project
- [ ] Create new Rust project with cargo
- [ ] Add actix-web, tokio, and other core dependencies
- [ ] Set up project structure (modules for routes, services, models)
- [ ] Configure logging (env_logger or tracing)
- [ ] Verify successful compilation
- [ ] Set up basic linting with clippy
- [ ] Fix any initial lint warnings

### Task 1.2: Initialize frontend structure
- [ ] Create frontend directory structure
- [ ] Set up TypeScript configuration (tsconfig.json)
- [ ] Add HTMX via CDN or npm
- [ ] Create basic HTML template structure
- [ ] Set up build tooling (esbuild or similar for TS compilation)
- [ ] Verify TypeScript compilation works
- [ ] Test basic HTMX functionality

### Task 1.3: Database setup
- [ ] Add SQLite dependencies (rusqlite or sqlx)
- [ ] Create database schema for vault configs and metadata
- [ ] Write migration system or initial schema setup
- [ ] Create database connection pool
- [ ] Test database connectivity
- [ ] Verify schema creation works
- [ ] Add database integration tests

### Task 1.4: Development environment
- [ ] Create .gitignore file
- [ ] Set up cargo watch for development
- [ ] Configure CORS for local development
- [ ] Create README with setup instructions
- [ ] Add example configuration file
- [ ] Document environment variables
- [ ] Test fresh setup on clean environment

## Feature 2: Core File System Operations

### Task 2.1: Vault configuration and management
- [ ] Create vault configuration model
- [ ] Implement vault registration endpoint (add new vault)
- [ ] Implement vault listing endpoint
- [ ] Implement vault deletion/unregistration
- [ ] Store vault configs in SQLite
- [ ] Validate vault paths exist and are accessible
- [ ] Test vault CRUD operations
- [ ] Add error handling for invalid paths

### Task 2.2: File tree browsing
- [ ] Create recursive directory traversal function
- [ ] Build file tree data structure
- [ ] Implement API endpoint for getting vault file tree
- [ ] Filter and categorize files (markdown vs other)
- [ ] Sort files and folders appropriately
- [ ] Handle symlinks and special files safely
- [ ] Test with various directory structures
- [ ] Add performance tests for large vaults

### Task 2.3: File reading
- [ ] Implement file read endpoint
- [ ] Handle different file encodings (UTF-8 primarily)
- [ ] Return appropriate content-type headers
- [ ] Add file size limits for safety
- [ ] Test reading various file types
- [ ] Handle permission errors gracefully
- [ ] Add integration tests

### Task 2.4: File creation
- [ ] Implement file creation endpoint
- [ ] Validate file paths and names
- [ ] Handle directory creation if needed
- [ ] Support creating markdown templates
- [ ] Prevent path traversal attacks
- [ ] Test file creation edge cases
- [ ] Verify proper error responses

### Task 2.5: File editing
- [ ] Implement file update endpoint
- [ ] Add file locking or version checking
- [ ] Handle concurrent edit detection
- [ ] Validate file content before writing
- [ ] Test various edit scenarios
- [ ] Verify atomic writes
- [ ] Add rollback on failure

### Task 2.6: File deletion
- [ ] Implement file deletion endpoint
- [ ] Add confirmation/safety checks
- [ ] Support moving to trash vs permanent delete
- [ ] Handle deletion of non-empty directories
- [ ] Test deletion permissions
- [ ] Verify cleanup of metadata
- [ ] Add restoration capability (optional)

### Task 2.7: Folder operations
- [ ] Implement folder creation endpoint
- [ ] Implement folder rename/move endpoint
- [ ] Implement folder deletion (recursive)
- [ ] Validate folder operations
- [ ] Test nested folder operations
- [ ] Handle move conflicts
- [ ] Add integration tests

## Feature 3: File Watching & Synchronization

### Task 3.1: File system watcher setup
- [ ] Add notify crate dependency
- [ ] Create file watcher service
- [ ] Configure recursive watching for vault paths
- [ ] Handle watcher errors and restarts
- [ ] Test watcher initialization
- [ ] Verify cross-platform compatibility
- [ ] Add logging for watch events

### Task 3.2: Change detection and processing
- [ ] Detect file creation events
- [ ] Detect file modification events
- [ ] Detect file deletion events
- [ ] Detect file rename/move events
- [ ] Debounce rapid changes (avoid duplicate events)
- [ ] Test all event types
- [ ] Handle batch operations efficiently

### Task 3.3: Web UI notification system
- [ ] Create WebSocket or SSE connection for real-time updates
- [ ] Send file change notifications to connected clients
- [ ] Implement client-side event handlers
- [ ] Update UI file tree on external changes
- [ ] Reload open files when changed externally
- [ ] Test notification delivery
- [ ] Handle connection drops gracefully

### Task 3.4: Conflict detection and resolution
- [ ] Track file modification timestamps
- [ ] Detect when web edit conflicts with disk change
- [ ] Create conflict backup files with naming convention
- [ ] Notify user of conflicts
- [ ] Allow user to choose resolution (optional UI)
- [ ] Test conflict scenarios
- [ ] Verify backup file creation

## Feature 4: Markdown Rendering & Editing

### Task 4.1: Markdown parsing and rendering
- [ ] Add markdown parsing library (pulldown-cmark or similar)
- [ ] Implement basic markdown to HTML conversion
- [ ] Add syntax highlighting for code blocks
- [ ] Support CommonMark specification
- [ ] Test various markdown features
- [ ] Verify rendering correctness
- [ ] Optimize rendering performance

### Task 4.2: Obsidian-specific syntax support
- [ ] Parse wiki-style links [[note]]
- [ ] Parse embed syntax ![[file]]
- [ ] Support block references [[note#^block]]
- [ ] Support header links [[note#header]]
- [ ] Parse tags #tag
- [ ] Support frontmatter (YAML)
- [ ] Test all Obsidian syntax variants
- [ ] Handle malformed syntax gracefully

### Task 4.3: Editor modes implementation
- [ ] Create raw markdown editor (textarea)
- [ ] Implement side-by-side mode (editor + preview)
- [ ] Implement formatted raw mode (syntax highlighting)
- [ ] Implement fully rendered mode (WYSIWYG-like)
- [ ] Add mode switching UI controls
- [ ] Persist user's mode preference
- [ ] Test all modes work correctly
- [ ] Optimize performance for large files

### Task 4.4: Editor features
- [ ] Add auto-save functionality
- [ ] Implement undo/redo
- [ ] Add keyboard shortcuts
- [ ] Support drag-and-drop for images/files
- [ ] Add insert link/image helpers
- [ ] Implement search within file (Ctrl+F)
- [ ] Test editor interactions
- [ ] Add accessibility features

### Task 4.5: Link resolution
- [ ] Resolve wiki links to actual file paths
- [ ] Handle ambiguous link names
- [ ] Support relative paths
- [ ] Enable click-to-navigate on links
- [ ] Show broken link indicators
- [ ] Test link resolution logic
- [ ] Handle special characters in links

## Feature 5: Search Functionality

### Task 5.1: Search indexing
- [ ] Create full-text search index structure
- [ ] Index markdown file content
- [ ] Index file and folder names
- [ ] Update index on file changes
- [ ] Implement incremental indexing
- [ ] Test indexing performance
- [ ] Add index rebuild capability

### Task 5.2: Search API
- [ ] Implement search endpoint
- [ ] Support full-text search
- [ ] Support file name search
- [ ] Add search result ranking
- [ ] Implement search filters (by folder, date, etc.)
- [ ] Test search accuracy
- [ ] Optimize search performance
- [ ] Add pagination for results

### Task 5.3: Search UI
- [ ] Create search input component
- [ ] Display search results with context
- [ ] Highlight matching terms
- [ ] Add click to open from results
- [ ] Show search result count
- [ ] Test search UX
- [ ] Add keyboard navigation

## Feature 6: User Interface

### Task 6.1: Layout and structure
- [ ] Create main application layout
- [ ] Implement resizable sidebar
- [ ] Create tab bar component
- [ ] Implement editor pane areas
- [ ] Add vault switcher UI
- [ ] Test responsive layout
- [ ] Verify accessibility

### Task 6.2: File tree sidebar
- [ ] Display hierarchical file tree
- [ ] Support expand/collapse folders
- [ ] Add file/folder icons
- [ ] Implement right-click context menu
- [ ] Support file/folder selection
- [ ] Add new file/folder buttons
- [ ] Test tree interactions
- [ ] Optimize rendering for large trees

### Task 6.3: Tab management
- [ ] Create tab component
- [ ] Support opening multiple files
- [ ] Implement tab switching
- [ ] Add tab close functionality
- [ ] Show unsaved changes indicator
- [ ] Persist open tabs
- [ ] Test tab lifecycle
- [ ] Handle edge cases (deleted files, etc.)

### Task 6.4: Split view functionality
- [ ] Implement split pane creation
- [ ] Support horizontal and vertical splits
- [ ] Enable drag-and-drop between splits
- [ ] Add split resizing
- [ ] Support up to 2+ panes
- [ ] Test split interactions
- [ ] Handle split close/merge
- [ ] Save split layout state

### Task 6.5: Styling and theming
- [ ] Create base CSS styles
- [ ] Implement light theme
- [ ] Implement dark theme
- [ ] Add theme switcher
- [ ] Ensure consistent styling
- [ ] Test color contrast (accessibility)
- [ ] Optimize CSS bundle size

### Task 6.6: HTMX integration
- [ ] Set up HTMX for dynamic updates
- [ ] Implement partial page updates
- [ ] Add loading indicators
- [ ] Handle HTMX errors gracefully
- [ ] Test HTMX interactions
- [ ] Optimize for minimal re-renders
- [ ] Add progress indicators

## Feature 7: Multi-Vault Support

### Task 7.1: Vault switching
- [ ] Create vault selector UI
- [ ] Implement vault switch endpoint
- [ ] Update file tree on vault change
- [ ] Close tabs from previous vault
- [ ] Update search index context
- [ ] Test vault switching
- [ ] Handle vault switching errors

### Task 7.2: Vault isolation
- [ ] Ensure proper path isolation
- [ ] Prevent cross-vault file access
- [ ] Separate search indices per vault
- [ ] Isolate file watchers per vault
- [ ] Test security boundaries
- [ ] Verify no data leakage
- [ ] Add vault access logging

## Feature 8: Configuration & Settings

### Task 8.1: Application configuration
- [ ] Create config file structure (TOML/JSON)
- [ ] Define server settings (port, host, etc.)
- [ ] Define vault default settings
- [ ] Implement config loading
- [ ] Support environment variable overrides
- [ ] Test config parsing
- [ ] Document all config options

### Task 8.2: User preferences
- [ ] Store UI preferences (theme, editor mode, etc.)
- [ ] Store recent files/folders
- [ ] Store window layout state
- [ ] Implement preferences API
- [ ] Test preferences persistence
- [ ] Handle preference migrations
- [ ] Add reset to defaults

## Feature 9: Error Handling & Logging

### Task 9.1: Error handling
- [ ] Create custom error types
- [ ] Implement consistent error responses
- [ ] Add user-friendly error messages
- [ ] Handle filesystem errors
- [ ] Handle database errors
- [ ] Test error scenarios
- [ ] Add error recovery where possible

### Task 9.2: Logging
- [ ] Set up structured logging
- [ ] Log all API requests
- [ ] Log file system operations
- [ ] Log errors with context
- [ ] Configure log levels
- [ ] Test log output
- [ ] Add log rotation

## Feature 10: Testing & Quality

### Task 10.1: Unit tests
- [ ] Write tests for file operations
- [ ] Write tests for vault management
- [ ] Write tests for search indexing
- [ ] Write tests for markdown parsing
- [ ] Write tests for conflict resolution
- [ ] Achieve >80% code coverage
- [ ] Fix failing tests

### Task 10.2: Integration tests
- [ ] Test full file CRUD workflows
- [ ] Test vault switching
- [ ] Test file watching and sync
- [ ] Test concurrent operations
- [ ] Test API endpoints
- [ ] Verify database operations
- [ ] Test error conditions

### Task 10.3: Performance testing
- [ ] Benchmark file tree loading
- [ ] Benchmark search performance
- [ ] Test with large vaults (10k+ files)
- [ ] Test with large files (10MB+)
- [ ] Profile memory usage
- [ ] Identify bottlenecks
- [ ] Optimize critical paths

### Task 10.4: Security testing
- [ ] Test path traversal prevention
- [ ] Test input validation
- [ ] Verify authentication (if added)
- [ ] Test CORS configuration
- [ ] Check for XSS vulnerabilities
- [ ] Review dependency vulnerabilities
- [ ] Run security audit

## Feature 11: Build & Deployment

### Task 11.1: Build process
- [ ] Configure release builds
- [ ] Optimize binary size
- [ ] Bundle frontend assets
- [ ] Set up cross-compilation (Linux, macOS, Windows)
- [ ] Test release builds
- [ ] Verify all features work in release mode
- [ ] Document build process

### Task 11.2: Standalone binary
- [ ] Embed frontend assets in binary
- [ ] Configure static file serving
- [ ] Test standalone execution
- [ ] Create installer/package scripts
- [ ] Test on fresh systems
- [ ] Document installation
- [ ] Create upgrade path

### Task 11.3: Docker support
- [ ] Create Dockerfile
- [ ] Optimize Docker image size
- [ ] Configure volume mounts for vaults
- [ ] Set up proper permissions
- [ ] Create docker-compose example
- [ ] Test Docker deployment
- [ ] Document Docker usage

### Task 11.4: Documentation
- [ ] Write user guide
- [ ] Document API endpoints
- [ ] Create architecture documentation
- [ ] Add troubleshooting guide
- [ ] Document configuration options
- [ ] Create contributing guide
- [ ] Add examples and screenshots

## Feature 12: Future Extensibility

### Task 12.1: Plugin architecture preparation
- [ ] Design plugin interface
- [ ] Create plugin loading mechanism
- [ ] Define plugin API contracts
- [ ] Add plugin discovery
- [ ] Document plugin development
- [ ] Create example plugin
- [ ] Test plugin system

### Task 12.2: Advanced features foundation
- [ ] Design graph view data structure
- [ ] Prepare for mobile responsive design
- [ ] Plan authentication system
- [ ] Design multi-user support
- [ ] Plan version control integration
- [ ] Document extension points
- [ ] Create roadmap
