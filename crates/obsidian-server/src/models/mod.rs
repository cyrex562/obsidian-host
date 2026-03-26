use chrono::{DateTime, Utc};
use sqlx::FromRow;

pub mod bookmarks;
pub mod graph;
pub mod plugin;

pub use obsidian_types::{
    AcceptInviteRequest, AddGroupMemberRequest, AdminUser, ApiKeyInfo, AuditLogEntry,
    BulkImportError, BulkImportResult, BulkUserEntry, ApplyChange,
    ApplyOrganizationSuggestionRequest,
    ApplyOrganizationSuggestionResponse, AuthenticatedUserProfile, ChangePasswordRequest,
    CreateApiKeyRequest, CreateApiKeyResponse, CreateFileRequest, CreateGroupRequest, CreateInviteRequest,
    CreateUploadSessionRequest, CreateUserRequest,
    CreateUserResponse, CreateVaultRequest, EditorMode, FileChangeEvent, FileChangeType,
    FileContent, FileNode, GenerateOrganizationSuggestionsRequest, GenerateOutlineRequest,
    GroupInfo, GroupMember, InviteInfo, MlUndoReceipt, NoteOutlineResponse, OrganizationSuggestion,
    OrganizationSuggestionKind, OrganizationSuggestionsResponse, OutlineSection, PagedSearchResult,
    ReverseAction, SearchMatch, SearchResult, SessionInfo, ShareVaultWithGroupRequest,
    ShareVaultWithUserRequest, TotpEnrollResponse, TotpVerifyRequest, UndoMlActionResponse,
    UpdateFileRequest, UploadSessionResponse,
    UserPreferences, Vault, VaultRole, VaultShareEntry, VaultShareList, WsMessage,
};

#[derive(Debug, Clone, FromRow)]
pub(crate) struct VaultRow {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<VaultRow> for Vault {
    fn from(row: VaultRow) -> Self {
        let path_exists = std::path::Path::new(&row.path).exists();
        Self {
            id: row.id,
            name: row.name,
            path: row.path,
            path_exists,
            created_at: DateTime::parse_from_rfc3339(&row.created_at)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            updated_at: DateTime::parse_from_rfc3339(&row.updated_at)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
        }
    }
}
