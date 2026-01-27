# API Documentation

Obsidian Host exposes a RESTful API for managing vaults and files.

## Base URL
Defaults to `http://localhost:8080/api`

## Authentication
Currently, no authentication is required.

## Response Format
Standard JSON responses.
-   Success: 200/201 OK
-   Error: 4xx/5xx with JSON body `{"error": "message"}`

## Endpoints

### Vaults

#### List Vaults
-   **GET** `/vaults`
-   Returns a list of registered vaults.

#### Create/Register Vault
-   **POST** `/vaults`
-   Body: `{"name": "My Vault", "path": "/absolute/common/path"}`
-   Registers a new vault with the system.

#### Get Vault Details
-   **GET** `/vaults/{id}`

#### Delete/Unregister Vault
-   **DELETE** `/vaults/{id}`

### Files

#### Get File Tree
-   **GET** `/vaults/{id}/files`
-   Returns the recreational file structure of the vault.

#### Get File Content
-   **GET** `/vaults/{id}/files/{path}`
-   `path` should be URL encoded.
-   Returns file content and metadata.

#### Get File Thumbnail
-   **GET** `/vaults/{id}/thumbnail/{path}?width=200&height=200`
-   Returns resized image (PNG).

#### Create File
-   **POST** `/vaults/{id}/files`
-   Body: `{"path": "folder/new_note.md", "content": "# Optional initial content"}`

#### Update File
-   **PUT** `/vaults/{id}/files/{path}`
-   Body: `{"content": "New content..."}`

#### Delete File
-   **DELETE** `/vaults/{id}/files/{path}`

#### Rename/Move File
-   **POST** `/vaults/{id}/files/move`
-   Body: `{"from": "old/path.md", "to": "new/path.md"}`

#### Upload File
-   **POST** `/vaults/{id}/upload`
-   Multipart form data. supports multiple files.

### Search

#### Search Vault
-   **GET** `/search/{vault_id}?q=query&limit=50`
-   Returns search results with match highlights.

### Preferences

#### Get Preferences
-   **GET** `/preferences`

#### Update Preferences
-   **PUT** `/preferences`
-   Body: `{"theme": "dark", "editor_mode": "side_by_side", ...}`

### Markdown

#### Render Markdown
-   **POST** `/markdown/render`
-   Body: `{"content": "# Markdown", "vault_id": "optional-id-for-links"}`
-   Returns HTML.
