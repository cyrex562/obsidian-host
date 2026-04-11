use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use codex_types::{
    AdminUser, ApplyOrganizationSuggestionRequest, ApplyOrganizationSuggestionResponse,
    CreateFileRequest, CreateUploadSessionRequest, CreateUserRequest, CreateUserResponse,
    CreateVaultRequest, FileChangeEvent, FileContent, FileNode,
    GenerateOrganizationSuggestionsRequest, GenerateOutlineRequest, NoteOutlineResponse,
    OrganizationSuggestionsResponse, PagedSearchResult, UndoMlActionResponse, UpdateFileRequest,
    UploadSessionResponse, UserPreferences, Vault,
};

pub type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[derive(Debug, Clone, Copy)]
enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("server error: {0}")]
    Server(String),

    #[error("api error: status={status}, message={message}")]
    ApiError { status: u16, message: String },

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("invalid header value for auth token")]
    InvalidAuthHeader,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameRequest {
    pub from: String,
    pub to: String,
    pub strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameResponse {
    pub new_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReindexResponse {
    pub indexed_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveWikiLinkRequest {
    pub link: String,
    pub current_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveWikiLinkResponse {
    pub path: String,
    pub exists: bool,
    pub ambiguous: bool,
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RandomNoteResponse {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyNoteRequest {
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFileRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadChunkResponse {
    pub uploaded_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginListResponse {
    pub plugins: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TogglePluginRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TogglePluginResponse {
    pub success: bool,
    pub plugin_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinishUploadSessionRequest {
    pub filename: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderRequest {
    pub content: String,
    pub current_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacklinkEntry {
    pub path: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagEntry {
    pub tag: String,
    pub count: usize,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClientDeployment {
    Cloud {
        base_url: String,
    },
    Standalone {
        base_url: String,
    },
    Hybrid {
        cloud_base_url: String,
        local_base_url: String,
        prefer_local_reads: bool,
    },
}

#[derive(Debug, Clone)]
pub struct ObsidianClient {
    base_url: String,
    deployment: ClientDeployment,
    inner: Client,
    auth: Arc<RwLock<AuthState>>,
}

#[derive(Debug, Clone, Default)]
struct AuthState {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_at_epoch_seconds: Option<u64>,
}

impl ObsidianClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self::for_cloud(base_url)
    }

    pub fn for_cloud(base_url: impl Into<String>) -> Self {
        Self::from_deployment(ClientDeployment::Cloud {
            base_url: base_url.into().trim_end_matches('/').to_string(),
        })
    }

    pub fn for_standalone(base_url: impl Into<String>) -> Self {
        Self::from_deployment(ClientDeployment::Standalone {
            base_url: base_url.into().trim_end_matches('/').to_string(),
        })
    }

    pub fn for_hybrid(
        cloud_base_url: impl Into<String>,
        local_base_url: impl Into<String>,
    ) -> Self {
        Self::from_deployment(ClientDeployment::Hybrid {
            cloud_base_url: cloud_base_url.into().trim_end_matches('/').to_string(),
            local_base_url: local_base_url.into().trim_end_matches('/').to_string(),
            prefer_local_reads: true,
        })
    }

    pub fn from_deployment(deployment: ClientDeployment) -> Self {
        let base_url = match &deployment {
            ClientDeployment::Cloud { base_url } | ClientDeployment::Standalone { base_url } => {
                base_url.clone()
            }
            ClientDeployment::Hybrid { cloud_base_url, .. } => cloud_base_url.clone(),
        };

        Self {
            base_url,
            deployment,
            inner: Client::new(),
            auth: Arc::new(RwLock::new(AuthState::default())),
        }
    }

    pub fn deployment(&self) -> &ClientDeployment {
        &self.deployment
    }

    pub fn mode_label(&self) -> &'static str {
        match self.deployment() {
            ClientDeployment::Cloud { .. } => "cloud",
            ClientDeployment::Standalone { .. } => "standalone",
            ClientDeployment::Hybrid { .. } => "hybrid",
        }
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.set_access_token(token);
        self
    }

    pub fn set_access_token(&mut self, access_token: impl Into<String>) {
        if let Ok(mut auth) = self.auth.write() {
            auth.access_token = Some(access_token.into());
            auth.expires_at_epoch_seconds = None;
        }
    }

    pub fn set_tokens(
        &mut self,
        access_token: impl Into<String>,
        refresh_token: impl Into<String>,
        expires_in_seconds: u64,
    ) {
        if let Ok(mut auth) = self.auth.write() {
            auth.access_token = Some(access_token.into());
            auth.refresh_token = Some(refresh_token.into());
            auth.expires_at_epoch_seconds =
                Some(now_epoch_seconds().saturating_add(expires_in_seconds));
        }
    }

    pub fn clear_access_token(&mut self) {
        if let Ok(mut auth) = self.auth.write() {
            auth.access_token = None;
            auth.refresh_token = None;
            auth.expires_at_epoch_seconds = None;
        }
    }

    pub fn refresh_token(&self) -> Option<String> {
        self.auth
            .read()
            .ok()
            .and_then(|auth| auth.refresh_token.clone())
    }

    pub fn access_token(&self) -> Option<String> {
        self.auth
            .read()
            .ok()
            .and_then(|auth| auth.access_token.clone())
    }

    pub async fn login(
        &mut self,
        username: &str,
        password: &str,
    ) -> Result<LoginResponse, ClientError> {
        let response: LoginResponse = self
            .send_json_with_options(
                HttpMethod::Post,
                "/api/auth/login",
                Some(&LoginRequest {
                    username: username.to_string(),
                    password: password.to_string(),
                }),
                false,
                false,
            )
            .await?;

        self.set_tokens(
            response.access_token.clone(),
            response.refresh_token.clone(),
            response.expires_in,
        );
        Ok(response)
    }

    pub async fn refresh_access_token(&self) -> Result<LoginResponse, ClientError> {
        let refresh_token = self
            .auth
            .read()
            .map_err(|_| ClientError::Server("auth state lock poisoned".to_string()))?
            .refresh_token
            .clone()
            .ok_or_else(|| ClientError::Server("missing refresh token".to_string()))?;

        let response: LoginResponse = self
            .send_json_raw(
                HttpMethod::Post,
                "/api/auth/refresh",
                Some(&RefreshTokenRequest { refresh_token }),
                false,
            )
            .await?;

        {
            let mut auth = self
                .auth
                .write()
                .map_err(|_| ClientError::Server("auth state lock poisoned".to_string()))?;
            auth.access_token = Some(response.access_token.clone());
            auth.refresh_token = Some(response.refresh_token.clone());
            auth.expires_at_epoch_seconds =
                Some(now_epoch_seconds().saturating_add(response.expires_in));
        }

        Ok(response)
    }

    pub async fn list_vaults(&self) -> Result<Vec<Vault>, ClientError> {
        self.send_json(HttpMethod::Get, "/api/vaults", Option::<&()>::None)
            .await
    }

    pub async fn create_vault(&self, request: &CreateVaultRequest) -> Result<Vault, ClientError> {
        self.send_json(HttpMethod::Post, "/api/vaults", Some(request))
            .await
    }

    pub async fn get_vault(&self, id: &str) -> Result<Vault, ClientError> {
        let endpoint = format!("/api/vaults/{id}");
        self.send_json(HttpMethod::Get, &endpoint, Option::<&()>::None)
            .await
    }

    pub async fn delete_vault(&self, id: &str) -> Result<(), ClientError> {
        let endpoint = format!("/api/vaults/{id}");
        self.send_no_content(HttpMethod::Delete, &endpoint).await
    }

    pub async fn get_file_tree(&self, vault_id: &str) -> Result<Vec<FileNode>, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/files");
        self.send_json(HttpMethod::Get, &endpoint, Option::<&()>::None)
            .await
    }

    pub async fn read_file(
        &self,
        vault_id: &str,
        file_path: &str,
    ) -> Result<FileContent, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/files/{file_path}");
        self.send_json(HttpMethod::Get, &endpoint, Option::<&()>::None)
            .await
    }

    pub async fn write_file(
        &self,
        vault_id: &str,
        file_path: &str,
        request: &UpdateFileRequest,
    ) -> Result<FileContent, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/files/{file_path}");
        self.send_json(HttpMethod::Put, &endpoint, Some(request))
            .await
    }

    pub async fn create_file(
        &self,
        vault_id: &str,
        request: &CreateFileRequest,
    ) -> Result<FileContent, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/files");
        self.send_json(HttpMethod::Post, &endpoint, Some(request))
            .await
    }

    pub async fn delete_file(&self, vault_id: &str, file_path: &str) -> Result<(), ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/files/{file_path}");
        self.send_no_content(HttpMethod::Delete, &endpoint).await
    }

    pub async fn create_directory(&self, vault_id: &str, path: &str) -> Result<(), ClientError> {
        #[derive(Serialize)]
        struct CreateDirectoryRequest<'a> {
            path: &'a str,
        }

        let endpoint = format!("/api/vaults/{vault_id}/directories");
        self.send_json::<serde_json::Value, _>(
            HttpMethod::Post,
            &endpoint,
            Some(&CreateDirectoryRequest { path }),
        )
        .await
        .map(|_| ())
    }

    pub async fn rename_file(
        &self,
        vault_id: &str,
        from: &str,
        to: &str,
        strategy: &str,
    ) -> Result<RenameResponse, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/rename");
        self.send_json(
            HttpMethod::Post,
            &endpoint,
            Some(&RenameRequest {
                from: from.to_string(),
                to: to.to_string(),
                strategy: strategy.to_string(),
            }),
        )
        .await
    }

    pub fn raw_file_url(&self, vault_id: &str, file_path: &str) -> String {
        format!("{}/api/vaults/{vault_id}/raw/{file_path}", self.base_url)
    }

    pub fn preferred_local_base_url(&self) -> Option<&str> {
        match self.deployment() {
            ClientDeployment::Cloud { .. } => None,
            ClientDeployment::Standalone { base_url } => Some(base_url.as_str()),
            ClientDeployment::Hybrid {
                local_base_url,
                prefer_local_reads,
                ..
            } => prefer_local_reads.then_some(local_base_url.as_str()),
        }
    }

    pub fn thumbnail_url(
        &self,
        vault_id: &str,
        file_path: &str,
        width: u32,
        height: u32,
    ) -> String {
        format!(
            "{}/api/vaults/{vault_id}/thumbnail/{file_path}?width={width}&height={height}",
            self.base_url
        )
    }

    pub async fn search(
        &self,
        vault_id: &str,
        query: &str,
        page: usize,
        page_size: usize,
    ) -> Result<PagedSearchResult, ClientError> {
        let endpoint = format!(
            "/api/vaults/{vault_id}/search?q={}&page={page}&page_size={page_size}",
            urlencoding::encode(query)
        );
        self.send_json(HttpMethod::Get, &endpoint, Option::<&()>::None)
            .await
    }

    pub async fn generate_outline(
        &self,
        vault_id: &str,
        file_path: &str,
        content: Option<&str>,
        max_sections: Option<usize>,
    ) -> Result<NoteOutlineResponse, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/ml/outline");
        self.send_json(
            HttpMethod::Post,
            &endpoint,
            Some(&GenerateOutlineRequest {
                file_path: file_path.to_string(),
                content: content.map(ToString::to_string),
                max_sections,
            }),
        )
        .await
    }

    pub async fn generate_suggestions(
        &self,
        vault_id: &str,
        file_path: &str,
        content: Option<&str>,
        max_suggestions: Option<usize>,
    ) -> Result<OrganizationSuggestionsResponse, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/ml/suggestions");
        self.send_json(
            HttpMethod::Post,
            &endpoint,
            Some(&GenerateOrganizationSuggestionsRequest {
                file_path: file_path.to_string(),
                content: content.map(ToString::to_string),
                max_suggestions,
            }),
        )
        .await
    }

    pub async fn apply_suggestion(
        &self,
        vault_id: &str,
        request: &ApplyOrganizationSuggestionRequest,
    ) -> Result<ApplyOrganizationSuggestionResponse, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/ml/apply-suggestion");
        self.send_json(HttpMethod::Post, &endpoint, Some(request))
            .await
    }

    pub async fn undo_ml_action(
        &self,
        vault_id: &str,
        receipt_id: &str,
    ) -> Result<UndoMlActionResponse, ClientError> {
        #[derive(Serialize)]
        struct UndoRequest<'a> {
            receipt_id: &'a str,
        }

        let endpoint = format!("/api/vaults/{vault_id}/ml/undo");
        self.send_json(
            HttpMethod::Post,
            &endpoint,
            Some(&UndoRequest { receipt_id }),
        )
        .await
    }

    pub async fn reindex(&self, vault_id: &str) -> Result<ReindexResponse, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/reindex");
        self.send_json(HttpMethod::Post, &endpoint, Option::<&()>::None)
            .await
    }

    pub async fn render_markdown(&self, content: &str) -> Result<String, ClientError> {
        self.send_text_with_options(
            HttpMethod::Post,
            "/api/render",
            Some(&RenderRequest {
                content: content.to_string(),
                current_file: None,
            }),
            false,
            false,
        )
        .await
    }

    pub async fn render_markdown_in_vault(
        &self,
        vault_id: &str,
        content: &str,
        current_file: Option<&str>,
    ) -> Result<String, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/render");
        self.send_text_with_options(
            HttpMethod::Post,
            &endpoint,
            Some(&RenderRequest {
                content: content.to_string(),
                current_file: current_file.map(ToString::to_string),
            }),
            true,
            true,
        )
        .await
    }

    pub async fn resolve_wiki_link(
        &self,
        vault_id: &str,
        link: &str,
        current_file: Option<&str>,
    ) -> Result<ResolveWikiLinkResponse, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/resolve-link");
        self.send_json(
            HttpMethod::Post,
            &endpoint,
            Some(&ResolveWikiLinkRequest {
                link: link.to_string(),
                current_file: current_file.map(ToString::to_string),
            }),
        )
        .await
    }

    pub async fn get_random_note(&self, vault_id: &str) -> Result<RandomNoteResponse, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/random");
        self.send_json(HttpMethod::Get, &endpoint, Option::<&()>::None)
            .await
    }

    pub async fn get_daily_note(
        &self,
        vault_id: &str,
        date: &str,
    ) -> Result<FileContent, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/daily");
        self.send_json(
            HttpMethod::Post,
            &endpoint,
            Some(&DailyNoteRequest {
                date: date.to_string(),
            }),
        )
        .await
    }

    pub async fn get_preferences(&self) -> Result<UserPreferences, ClientError> {
        self.send_json(HttpMethod::Get, "/api/preferences", Option::<&()>::None)
            .await
    }

    pub async fn update_preferences(
        &self,
        prefs: &UserPreferences,
    ) -> Result<UserPreferences, ClientError> {
        self.send_json(HttpMethod::Put, "/api/preferences", Some(prefs))
            .await
    }

    pub async fn reset_preferences(&self) -> Result<UserPreferences, ClientError> {
        self.send_json(
            HttpMethod::Post,
            "/api/preferences/reset",
            Option::<&()>::None,
        )
        .await
    }

    pub async fn get_recent_files(&self, vault_id: &str) -> Result<Vec<String>, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/recent");
        self.send_json(HttpMethod::Get, &endpoint, Option::<&()>::None)
            .await
    }

    pub async fn get_backlinks(
        &self,
        vault_id: &str,
        path: &str,
    ) -> Result<Vec<BacklinkEntry>, ClientError> {
        let endpoint = format!(
            "/api/vaults/{vault_id}/backlinks?path={}",
            urlencoding::encode(path)
        );
        self.send_json(HttpMethod::Get, &endpoint, Option::<&()>::None)
            .await
    }

    pub async fn get_tags(&self, vault_id: &str) -> Result<Vec<TagEntry>, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/tags");
        self.send_json(HttpMethod::Get, &endpoint, Option::<&()>::None)
            .await
    }

    /// Fetch file change events since a given Unix millisecond timestamp.
    pub async fn get_file_changes(
        &self,
        vault_id: &str,
        since_ms: i64,
    ) -> Result<Vec<FileChangeEvent>, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/changes?since={since_ms}");
        self.send_json(HttpMethod::Get, &endpoint, Option::<&()>::None)
            .await
    }

    pub async fn record_recent_file(&self, vault_id: &str, path: &str) -> Result<(), ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/recent");
        self.send_json::<serde_json::Value, _>(
            HttpMethod::Post,
            &endpoint,
            Some(&RecentFileRequest {
                path: path.to_string(),
            }),
        )
        .await
        .map(|_| ())
    }

    pub async fn create_upload_session(
        &self,
        vault_id: &str,
        request: &CreateUploadSessionRequest,
    ) -> Result<UploadSessionResponse, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/upload-sessions");
        self.send_json(HttpMethod::Post, &endpoint, Some(request))
            .await
    }

    pub async fn upload_chunk(
        &self,
        vault_id: &str,
        session_id: &str,
        chunk: Vec<u8>,
    ) -> Result<UploadChunkResponse, ClientError> {
        self.ensure_token_fresh().await?;

        let endpoint = format!(
            "{}/api/vaults/{vault_id}/upload-sessions/{session_id}",
            self.base_url
        );
        let mut req = self.inner.put(endpoint).body(chunk);
        if let Some(token) = self.current_access_token()? {
            req = req.headers(Self::auth_header(&token)?);
        }
        let response = req.send().await?;
        let status = response.status();
        if !status.is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "request failed".to_string());
            return Err(ClientError::ApiError {
                status: status.as_u16(),
                message,
            });
        }
        Ok(response.json::<UploadChunkResponse>().await?)
    }

    pub async fn finish_upload_session(
        &self,
        vault_id: &str,
        session_id: &str,
        request: &FinishUploadSessionRequest,
    ) -> Result<serde_json::Value, ClientError> {
        let endpoint = format!("/api/vaults/{vault_id}/upload-sessions/{session_id}/finish");
        self.send_json(HttpMethod::Post, &endpoint, Some(request))
            .await
    }

    pub fn download_file_url(&self, vault_id: &str, file_path: &str) -> String {
        format!(
            "{}/api/vaults/{vault_id}/download/{file_path}",
            self.base_url
        )
    }

    pub async fn list_plugins(&self) -> Result<PluginListResponse, ClientError> {
        self.send_json(HttpMethod::Get, "/api/plugins", Option::<&()>::None)
            .await
    }

    pub async fn download_file_bytes(
        &self,
        vault_id: &str,
        file_path: &str,
    ) -> Result<Vec<u8>, ClientError> {
        self.ensure_token_fresh().await?;

        let url = self.download_file_url(vault_id, file_path);
        let mut req = self.inner.get(&url);
        if let Some(token) = self.current_access_token()? {
            req = req.headers(Self::auth_header(&token)?);
        }
        let response = req.send().await?;
        let status = response.status();
        if !status.is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "request failed".to_string());
            return Err(ClientError::ApiError {
                status: status.as_u16(),
                message,
            });
        }
        Ok(response.bytes().await?.to_vec())
    }

    pub async fn toggle_plugin(
        &self,
        plugin_id: &str,
        enabled: bool,
    ) -> Result<TogglePluginResponse, ClientError> {
        let endpoint = format!("/api/plugins/{plugin_id}/toggle");
        self.send_json(
            HttpMethod::Post,
            &endpoint,
            Some(&TogglePluginRequest { enabled }),
        )
        .await
    }

    // ── Admin methods ────────────────────────────────────────────────

    pub async fn admin_list_users(&self) -> Result<Vec<AdminUser>, ClientError> {
        self.send_json(HttpMethod::Get, "/api/admin/users", Option::<&()>::None)
            .await
    }

    pub async fn admin_create_user(
        &self,
        request: &CreateUserRequest,
    ) -> Result<CreateUserResponse, ClientError> {
        self.send_json(HttpMethod::Post, "/api/admin/users", Some(request))
            .await
    }

    pub async fn admin_edit_user(
        &self,
        user_id: &str,
        is_admin: Option<bool>,
        reset_password: Option<&str>,
    ) -> Result<serde_json::Value, ClientError> {
        let endpoint = format!("/api/admin/users/{user_id}/edit");
        self.send_json(
            HttpMethod::Post,
            &endpoint,
            Some(&serde_json::json!({
                "is_admin": is_admin,
                "reset_password": reset_password,
            })),
        )
        .await
    }

    pub async fn admin_deactivate_user(
        &self,
        user_id: &str,
    ) -> Result<serde_json::Value, ClientError> {
        let endpoint = format!("/api/admin/users/{user_id}/deactivate");
        self.send_json(HttpMethod::Post, &endpoint, Option::<&()>::None)
            .await
    }

    pub async fn admin_reactivate_user(
        &self,
        user_id: &str,
    ) -> Result<serde_json::Value, ClientError> {
        let endpoint = format!("/api/admin/users/{user_id}/reactivate");
        self.send_json(HttpMethod::Post, &endpoint, Option::<&()>::None)
            .await
    }

    pub async fn admin_delete_user(&self, user_id: &str) -> Result<serde_json::Value, ClientError> {
        let endpoint = format!("/api/admin/users/{user_id}");
        self.send_json(HttpMethod::Delete, &endpoint, Option::<&()>::None)
            .await
    }

    pub async fn connect_ws(&self) -> Result<WsStream, ClientError> {
        self.ensure_token_fresh().await?;

        let base = self.base_url.trim_end_matches('/');
        let ws_base = if let Some(stripped) = base.strip_prefix("https://") {
            format!("wss://{stripped}")
        } else if let Some(stripped) = base.strip_prefix("http://") {
            format!("ws://{stripped}")
        } else {
            format!("ws://{base}")
        };

        let ws_url = format!("{ws_base}/api/ws");
        let mut request = ws_url.into_client_request()?;

        if let Some(token) = self.current_access_token()? {
            let header = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|_| ClientError::InvalidAuthHeader)?;
            request.headers_mut().insert(AUTHORIZATION, header);
        }

        let (stream, _) = tokio_tungstenite::connect_async(request).await?;
        Ok(stream)
    }

    async fn send_json<T, B>(
        &self,
        method: HttpMethod,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, ClientError>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        self.send_json_with_options(method, path, body, true, true)
            .await
    }

    async fn send_json_with_options<T, B>(
        &self,
        method: HttpMethod,
        path: &str,
        body: Option<&B>,
        attach_auth_header: bool,
        auto_refresh_token: bool,
    ) -> Result<T, ClientError>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        if auto_refresh_token {
            self.ensure_token_fresh().await?;
        }

        self.send_json_raw(method, path, body, attach_auth_header)
            .await
    }

    async fn send_json_raw<T, B>(
        &self,
        method: HttpMethod,
        path: &str,
        body: Option<&B>,
        attach_auth_header: bool,
    ) -> Result<T, ClientError>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        let url = format!("{}{}", self.base_url, path);

        let mut request = match method {
            HttpMethod::Get => self.inner.get(&url),
            HttpMethod::Post => self.inner.post(&url),
            HttpMethod::Put => self.inner.put(&url),
            HttpMethod::Delete => self.inner.delete(&url),
        };

        if attach_auth_header {
            if let Some(token) = self.current_access_token()? {
                request = request.headers(Self::auth_header(&token)?);
            }
        }

        if let Some(payload) = body {
            request = request.json(payload);
        }

        let response = request.send().await?;
        let status = response.status();

        if !status.is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "request failed".to_string());
            return Err(ClientError::ApiError {
                status: status.as_u16(),
                message,
            });
        }

        Ok(response.json::<T>().await?)
    }

    async fn send_text_with_options<B>(
        &self,
        method: HttpMethod,
        path: &str,
        body: Option<&B>,
        attach_auth_header: bool,
        auto_refresh_token: bool,
    ) -> Result<String, ClientError>
    where
        B: Serialize + ?Sized,
    {
        if auto_refresh_token {
            self.ensure_token_fresh().await?;
        }

        self.send_text_raw(method, path, body, attach_auth_header)
            .await
    }

    async fn send_text_raw<B>(
        &self,
        method: HttpMethod,
        path: &str,
        body: Option<&B>,
        attach_auth_header: bool,
    ) -> Result<String, ClientError>
    where
        B: Serialize + ?Sized,
    {
        let url = format!("{}{}", self.base_url, path);

        let mut request = match method {
            HttpMethod::Get => self.inner.get(&url),
            HttpMethod::Post => self.inner.post(&url),
            HttpMethod::Put => self.inner.put(&url),
            HttpMethod::Delete => self.inner.delete(&url),
        };

        if attach_auth_header {
            if let Some(token) = self.current_access_token()? {
                request = request.headers(Self::auth_header(&token)?);
            }
        }

        if let Some(payload) = body {
            request = request.json(payload);
        }

        let response = request.send().await?;
        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            return Err(ClientError::ApiError {
                status: status.as_u16(),
                message: text,
            });
        }

        Ok(text)
    }

    async fn send_no_content(&self, method: HttpMethod, path: &str) -> Result<(), ClientError> {
        self.ensure_token_fresh().await?;

        let url = format!("{}{}", self.base_url, path);

        let mut request = match method {
            HttpMethod::Get => self.inner.get(&url),
            HttpMethod::Post => self.inner.post(&url),
            HttpMethod::Put => self.inner.put(&url),
            HttpMethod::Delete => self.inner.delete(&url),
        };

        if let Some(token) = self.current_access_token()? {
            request = request.headers(Self::auth_header(&token)?);
        }

        let response = request.send().await?;
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }

        let message = response
            .text()
            .await
            .unwrap_or_else(|_| "request failed".to_string());
        Err(ClientError::ApiError {
            status: status.as_u16(),
            message,
        })
    }

    fn auth_header(token: &str) -> Result<HeaderMap, ClientError> {
        let mut headers = HeaderMap::new();
        let value = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| ClientError::InvalidAuthHeader)?;
        headers.insert(AUTHORIZATION, value);
        Ok(headers)
    }

    fn current_access_token(&self) -> Result<Option<String>, ClientError> {
        let auth = self
            .auth
            .read()
            .map_err(|_| ClientError::Server("auth state lock poisoned".to_string()))?;
        Ok(auth.access_token.clone())
    }

    async fn ensure_token_fresh(&self) -> Result<(), ClientError> {
        let should_refresh = {
            let auth = self
                .auth
                .read()
                .map_err(|_| ClientError::Server("auth state lock poisoned".to_string()))?;

            if auth.access_token.is_none() {
                false
            } else if let Some(exp) = auth.expires_at_epoch_seconds {
                exp <= now_epoch_seconds().saturating_add(60)
            } else {
                false
            }
        };

        if should_refresh {
            let _ = self.refresh_access_token().await?;
        }

        Ok(())
    }
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
