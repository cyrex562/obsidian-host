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

## Feature 13: Multi-Media File Support

### Task 13.1: Image viewing and handling
- [ ] Add image file detection (png, jpg, jpeg, gif, svg, webp)
- [ ] Create image viewer component in UI
- [ ] Implement image thumbnail generation
- [ ] Add image lazy loading for performance
- [ ] Support image zoom and pan
- [ ] Display image metadata (dimensions, size, type)
- [ ] Test with various image formats

### Task 13.2: PDF viewing
- [ ] Add PDF file detection
- [ ] Integrate PDF.js or similar viewer
- [ ] Implement PDF page navigation
- [ ] Add PDF search functionality
- [ ] Support PDF zoom controls
- [ ] Display PDF metadata
- [ ] Test with various PDF files

### Task 13.3: Other file type support
- [ ] Add audio file playback (mp3, wav, ogg)
- [ ] Add video file playback (mp4, webm)
- [ ] Support code file syntax highlighting (js, py, rs, etc)
- [ ] Add text file viewing for non-markdown
- [ ] Create fallback download option for unsupported types
- [ ] Test with various file types
- [ ] Document supported file types

### Task 13.4: File preview in sidebar
- [ ] Add hover preview for images
- [ ] Show file type icons
- [ ] Display file size and metadata
- [ ] Add quick preview panel
- [ ] Test preview performance
- [ ] Optimize for large files
- [ ] Add preview caching

## Feature 14: File Upload & Download

### Task 14.1: Single file upload
- [ ] Create upload API endpoint
- [ ] Add drag-and-drop upload to UI
- [ ] Implement file upload button
- [ ] Add upload progress indicator
- [ ] Validate file types and sizes
- [ ] Handle upload errors gracefully
- [ ] Test with various file types and sizes

### Task 14.2: Multiple file upload
- [ ] Support multiple file selection
- [ ] Implement batch upload queue
- [ ] Add individual file progress indicators
- [ ] Support drag-and-drop multiple files
- [ ] Handle partial upload failures
- [ ] Add upload resume capability
- [ ] Test with large batches

### Task 14.3: Folder upload
- [ ] Add folder upload support
- [ ] Preserve directory structure
- [ ] Show folder upload progress
- [ ] Handle nested folders
- [ ] Validate total upload size
- [ ] Test with complex folder structures
- [ ] Add folder upload UI

### Task 14.4: Download functionality
- [ ] Implement single file download
- [ ] Add folder download as zip
- [ ] Support multiple file selection download
- [ ] Create zip compression service
- [ ] Add download progress for large files
- [ ] Include metadata in downloads
- [ ] Test download performance

### Task 14.5: Bulk operations UI
- [ ] Add checkbox selection to file tree
- [ ] Create bulk action toolbar
- [ ] Support select all/none/invert
- [ ] Add context menu for selections
- [ ] Implement bulk delete with confirmation
- [ ] Add bulk move/rename
- [ ] Test bulk operations performance

## Feature 15: Server-Side Plugin System

### Task 15.1: Plugin architecture design
- [ ] Design plugin trait/interface
- [ ] Define plugin lifecycle hooks
- [ ] Create plugin manifest format (TOML/JSON)
- [ ] Design plugin sandboxing strategy
- [ ] Plan plugin API surface
- [ ] Document plugin capabilities
- [ ] Create plugin security model

### Task 15.2: Plugin loading mechanism
- [ ] Implement dynamic plugin loading (WASM or native)
- [ ] Create plugin discovery system
- [ ] Add plugin dependency resolution
- [ ] Implement plugin version checking
- [ ] Create plugin registry/catalog
- [ ] Add plugin enable/disable functionality
- [ ] Test plugin isolation

### Task 15.3: Plugin API implementation
- [ ] Expose file system operations to plugins
- [ ] Provide markdown parsing utilities
- [ ] Add event system for plugins
- [ ] Create plugin settings storage
- [ ] Implement plugin-to-plugin communication
- [ ] Add plugin UI extension points
- [ ] Document plugin API

### Task 15.4: Core plugins
- [ ] Implement Daily Notes plugin
- [ ] Create Templates plugin
- [ ] Build Backlinks plugin
- [ ] Add Tag browser plugin
- [ ] Create Outline/TOC plugin
- [ ] Build Word count plugin
- [ ] Test all core plugins

### Task 15.5: Plugin management UI
- [ ] Create plugin marketplace/browser
- [ ] Add plugin installation UI
- [ ] Implement plugin settings page
- [ ] Show plugin status and logs
- [ ] Add plugin update notifications
- [ ] Create plugin developer tools
- [ ] Test plugin UI workflows

## Feature 16: Obsidian-Native Features

### Task 16.1: Daily Notes
- [ ] Implement daily note creation logic
- [ ] Add configurable daily note template
- [ ] Create daily note naming convention settings
- [ ] Add calendar picker for date selection
- [ ] Implement "Open today's note" command
- [ ] Support custom date formats
- [ ] Test daily note workflow

### Task 16.2: Random Note
- [ ] Implement random note selection algorithm
- [ ] Add "Random Note" button to UI
- [ ] Support filtering (e.g., only certain folders)
- [ ] Add keyboard shortcut
- [ ] Weight by recent edits or tags
- [ ] Test randomization fairness
- [ ] Add random note API endpoint

### Task 16.3: Calendar integration
- [ ] Create calendar view component
- [ ] Show notes by creation/modification date
- [ ] Highlight days with notes
- [ ] Support date-based note navigation
- [ ] Add calendar in sidebar panel
- [ ] Integrate with daily notes
- [ ] Test calendar performance

### Task 16.4: Templates system
- [ ] Create template storage location
- [ ] Implement template variable substitution
- [ ] Add template insertion UI
- [ ] Support template snippets
- [ ] Create default templates
- [ ] Add template creation from note
- [ ] Test template functionality

### Task 16.5: Quick switcher
- [ ] Implement fuzzy file search
- [ ] Create quick switcher modal (Cmd/Ctrl+O)
- [ ] Add recent files list
- [ ] Support file creation from switcher
- [ ] Show file path and preview
- [ ] Add keyboard navigation
- [ ] Test switcher performance

## Feature 17: Canvas View

### Task 17.1: Canvas data model
- [ ] Design canvas file format (.canvas)
- [ ] Create node and edge data structures
- [ ] Implement canvas serialization
- [ ] Add support for different node types
- [ ] Design canvas metadata
- [ ] Create canvas validation
- [ ] Test canvas persistence

### Task 17.2: Canvas rendering
- [ ] Create canvas viewport component
- [ ] Implement pan and zoom controls
- [ ] Render note nodes with previews
- [ ] Draw connections between nodes
- [ ] Add node positioning system
- [ ] Implement canvas background grid
- [ ] Optimize rendering performance

### Task 17.3: Canvas editing
- [ ] Add drag-and-drop nodes
- [ ] Implement node resizing
- [ ] Create edge drawing tool
- [ ] Add text/media nodes
- [ ] Support node grouping
- [ ] Implement undo/redo for canvas
- [ ] Test canvas editing workflow

### Task 17.4: Canvas-note integration
- [ ] Add "Add to canvas" option for notes
- [ ] Create canvas from selected notes
- [ ] Support embedding canvas in notes
- [ ] Implement bi-directional links
- [ ] Add canvas thumbnail previews
- [ ] Support canvas templates
- [ ] Test integration points

### Task 17.5: Graph view
- [ ] Create force-directed graph layout
- [ ] Show all notes as graph nodes
- [ ] Display wiki link connections
- [ ] Add filtering and highlighting
- [ ] Implement graph navigation
- [ ] Support graph export
- [ ] Test with large note collections

## Feature 18: Metadata Management

### Task 18.1: Frontmatter editing
- [ ] Parse YAML frontmatter
- [ ] Create frontmatter editor UI
- [ ] Support key-value pair editing
- [ ] Add frontmatter templates
- [ ] Validate frontmatter syntax
- [ ] Support arrays and nested objects
- [ ] Test frontmatter parsing

### Task 18.2: Property types
- [ ] Support text properties
- [ ] Add number properties
- [ ] Implement date/datetime properties
- [ ] Support tag/multi-tag properties
- [ ] Add link/multi-link properties
- [ ] Create checkbox properties
- [ ] Test all property types

### Task 18.3: Metadata views
- [ ] Create properties panel in UI
- [ ] Show all properties for current note
- [ ] Add inline property editing
- [ ] Support property search/filter
- [ ] Display property statistics
- [ ] Create property auto-complete
- [ ] Test metadata display

### Task 18.4: Property-based features
- [ ] Implement property-based search
- [ ] Create property-based note sorting
- [ ] Add property templates
- [ ] Support property inheritance
- [ ] Create property views/tables
- [ ] Add property validation rules
- [ ] Test property queries

### Task 18.5: Tags system
- [ ] Implement tag parsing from content
- [ ] Create tag browser/explorer
- [ ] Add tag auto-complete
- [ ] Support nested tags (tag/subtag)
- [ ] Create tag-based search
- [ ] Show tag counts and usage
- [ ] Test tag functionality

## Feature 19: Enhanced Search & Organization

### Task 19.1: Advanced search operators
- [ ] Support boolean operators (AND, OR, NOT)
- [ ] Add field-specific search (title:, content:, tag:)
- [ ] Implement regex search
- [ ] Add date range search
- [ ] Support property-based queries
- [ ] Create search query builder UI
- [ ] Test complex queries

### Task 19.2: Saved searches
- [ ] Implement search saving mechanism
- [ ] Create saved search UI
- [ ] Add search history
- [ ] Support search sharing/export
- [ ] Create search shortcuts
- [ ] Add search notifications
- [ ] Test saved search persistence

### Task 19.3: Note collections
- [ ] Create manual note collections
- [ ] Implement smart collections (dynamic queries)
- [ ] Add collection management UI
- [ ] Support collection nesting
- [ ] Create collection views
- [ ] Add collection export
- [ ] Test collection performance

### Task 19.4: Dataview-like queries
- [ ] Implement query language
- [ ] Support table/list/task views
- [ ] Add aggregation functions
- [ ] Create query editor
- [ ] Support query embedding in notes
- [ ] Add query result caching
- [ ] Test query performance
