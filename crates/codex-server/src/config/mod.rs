use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_DEV_JWT_SECRET: &str = "dev-insecure-jwt-secret-change-me";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub vault: VaultConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub sync: SyncConfig,
    #[serde(default)]
    pub cors: CorsConfig,
    #[serde(default)]
    pub tls: TlsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    #[serde(default = "default_vault_base_dir")]
    pub base_dir: String,

    #[serde(default = "default_exclusions")]
    pub index_exclusions: Vec<String>,

    /// Default document format for newly created vaults. Currently only
    /// `"markdown"` is supported; reserved for a future `"mdx"` format.
    #[serde(default = "default_document_format")]
    pub document_format: String,
}

fn default_document_format() -> String {
    "markdown".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_auth_enabled")]
    pub enabled: bool,
    /// Authentication provider. Valid values: `"password"` (default), `"ldap"`, `"oidc"`.
    ///
    /// Note: `"mtls"` was removed — mutual TLS requires a reverse proxy to extract and
    /// forward the client certificate; it cannot be implemented as an application-layer
    /// auth provider.
    #[serde(default = "default_auth_provider")]
    pub provider: String,
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default = "default_access_token_ttl")]
    pub access_token_ttl: u64,
    #[serde(default = "default_refresh_token_ttl")]
    pub refresh_token_ttl: u64,
    #[serde(default)]
    pub bootstrap_admin_username: Option<String>,
    #[serde(default)]
    pub bootstrap_admin_password: Option<String>,

    // Password policy settings
    #[serde(default = "default_min_password_length")]
    pub min_password_length: usize,
    #[serde(default)]
    pub require_uppercase: bool,
    #[serde(default)]
    pub require_lowercase: bool,
    #[serde(default)]
    pub require_digit: bool,
    #[serde(default)]
    pub require_special: bool,
    /// Maximum failed login attempts before lockout (0 = disabled).
    #[serde(default = "default_max_failed_logins")]
    pub max_failed_logins: u64,
    /// Lockout duration in minutes after exceeding failed attempts.
    #[serde(default = "default_lockout_minutes")]
    pub lockout_minutes: u64,

    // ── OIDC settings ───────────────────────────────────────────────
    /// OIDC provider discovery URL (e.g. https://accounts.google.com).
    #[serde(default)]
    pub oidc_issuer_url: Option<String>,
    /// OIDC client ID.
    #[serde(default)]
    pub oidc_client_id: Option<String>,
    /// OIDC client secret.
    #[serde(default)]
    pub oidc_client_secret: Option<String>,
    /// URL the provider redirects back to after auth (e.g. http://localhost:8080/api/auth/oidc/callback).
    #[serde(default)]
    pub oidc_redirect_uri: Option<String>,

    // ── LDAP settings ───────────────────────────────────────────────
    /// LDAP server URL (e.g. ldap://ldap.example.com:389).
    #[serde(default)]
    pub ldap_url: Option<String>,
    /// Base DN for user search (e.g. ou=people,dc=example,dc=com).
    #[serde(default)]
    pub ldap_base_dn: Option<String>,
    /// Bind DN for the service account (e.g. cn=admin,dc=example,dc=com).
    #[serde(default)]
    pub ldap_bind_dn: Option<String>,
    /// Bind password for the service account.
    #[serde(default)]
    pub ldap_bind_password: Option<String>,
    /// LDAP attribute that contains the username (default: uid).
    #[serde(default = "default_ldap_user_attr")]
    pub ldap_user_attr: String,
    /// LDAP search filter template. Use {username} as placeholder.
    #[serde(default = "default_ldap_search_filter")]
    pub ldap_search_filter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    #[serde(default = "default_change_log_retention_days")]
    pub change_log_retention_days: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    #[serde(default = "default_cors_allowed_origins")]
    pub allowed_origins: Vec<String>,
}

/// Optional TLS configuration for the HTTP server.
///
/// When both `cert_file` and `key_file` are provided the server binds with
/// TLS (HTTPS) instead of plain HTTP.  Both files must be PEM-encoded.
/// If only one is set the server will refuse to start.
///
/// Example `config.toml`:
/// ```toml
/// [tls]
/// cert_file = "/etc/codex/tls/server.crt"
/// key_file  = "/etc/codex/tls/server.key"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TlsConfig {
    /// Path to the PEM-encoded TLS certificate (or chain).
    #[serde(default)]
    pub cert_file: Option<String>,
    /// Path to the PEM-encoded private key.
    #[serde(default)]
    pub key_file: Option<String>,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_db_path() -> String {
    "./codex.db".to_string()
}

fn default_exclusions() -> Vec<String> {
    vec![
        ".git".to_string(),
        ".obsidian".to_string(),
        ".trash".to_string(),
        "node_modules".to_string(),
    ]
}

fn default_vault_base_dir() -> String {
    "./vaults".to_string()
}

fn default_auth_enabled() -> bool {
    false
}

fn default_jwt_secret() -> String {
    "".to_string()
}

fn default_auth_provider() -> String {
    "password".to_string()
}

fn default_access_token_ttl() -> u64 {
    3600
}

fn default_refresh_token_ttl() -> u64 {
    604800
}

fn default_change_log_retention_days() -> u64 {
    7
}

fn default_cors_allowed_origins() -> Vec<String> {
    vec!["http://localhost:5173".to_string()]
}

fn default_ldap_user_attr() -> String {
    "uid".to_string()
}

fn default_ldap_search_filter() -> String {
    "(&(objectClass=inetOrgPerson)({attr}={username}))".to_string()
}

fn default_min_password_length() -> usize {
    12
}

fn default_max_failed_logins() -> u64 {
    5
}

fn default_lockout_minutes() -> u64 {
    15
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: default_host(),
                port: default_port(),
            },
            database: DatabaseConfig {
                path: default_db_path(),
            },
            vault: VaultConfig {
                base_dir: default_vault_base_dir(),
                index_exclusions: default_exclusions(),
                document_format: default_document_format(),
            },
            auth: AuthConfig::default(),
            sync: SyncConfig {
                change_log_retention_days: default_change_log_retention_days(),
            },
            cors: CorsConfig {
                allowed_origins: default_cors_allowed_origins(),
            },
            tls: TlsConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_db_path(),
        }
    }
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            base_dir: default_vault_base_dir(),
            index_exclusions: default_exclusions(),
            document_format: default_document_format(),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: default_auth_enabled(),
            provider: default_auth_provider(),
            jwt_secret: default_jwt_secret(),
            access_token_ttl: default_access_token_ttl(),
            refresh_token_ttl: default_refresh_token_ttl(),
            bootstrap_admin_username: None,
            bootstrap_admin_password: None,
            min_password_length: default_min_password_length(),
            require_uppercase: false,
            require_lowercase: false,
            require_digit: false,
            require_special: false,
            max_failed_logins: default_max_failed_logins(),
            lockout_minutes: default_lockout_minutes(),
            oidc_issuer_url: None,
            oidc_client_id: None,
            oidc_client_secret: None,
            oidc_redirect_uri: None,
            ldap_url: None,
            ldap_base_dn: None,
            ldap_bind_dn: None,
            ldap_bind_password: None,
            ldap_user_attr: default_ldap_user_attr(),
            ldap_search_filter: default_ldap_search_filter(),
        }
    }
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            change_log_retention_days: default_change_log_retention_days(),
        }
    }
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: default_cors_allowed_origins(),
        }
    }
}

/// Platform-standard directory set passed to `AppConfig` when running as the
/// Tauri desktop app. Standalone server (`--config` flag) does not use this.
#[derive(Debug, Clone)]
pub struct CodexPaths {
    /// Directory that holds `config.toml`. Maps to `app_config_dir()` in Tauri.
    pub config_dir: PathBuf,
    /// Directory for the database, plugins, and logs. Maps to `app_data_dir()`.
    pub data_dir: PathBuf,
    /// Cache directory. Maps to `app_cache_dir()`.
    pub cache_dir: PathBuf,
    /// Default location where the user's vault(s) live.
    pub default_vault_dir: PathBuf,
}

impl AppConfig {
    /// Load from an explicit file path (standalone server / `--config` flag).
    ///
    /// Returns a descriptive error if the file is missing or cannot be parsed.
    /// Environment variable overrides (`CODEX__*`) are applied on top.
    pub fn load_from_file(path: PathBuf) -> anyhow::Result<Self> {
        if !path.exists() {
            anyhow::bail!("Config file not found: {}", path.display());
        }
        let path_str = path
            .to_str()
            .context("Config path contains non-UTF-8 characters")?;
        let cfg = config::Config::builder()
            .add_source(config::File::from_str(
                &serde_json::to_string(&AppConfig::default()).unwrap(),
                config::FileFormat::Json,
            ))
            .add_source(
                config::File::new(path_str, config::FileFormat::Toml).required(true),
            )
            .add_source(config::Environment::with_prefix("CODEX").separator("__"))
            .build()
            .with_context(|| format!("Failed to load config from {}", path.display()))?;
        cfg.try_deserialize()
            .with_context(|| format!("Failed to parse config from {}", path.display()))
    }

    /// Load using Tauri platform directories.
    ///
    /// Reads `{config_dir}/config.toml` when it exists; otherwise returns the
    /// struct produced by `default_for_dirs` (no error).
    pub fn load_from_dirs(paths: &CodexPaths) -> anyhow::Result<Self> {
        let config_file = paths.config_dir.join("config.toml");
        if config_file.exists() {
            Self::load_from_file(config_file)
        } else {
            Ok(Self::default_for_dirs(paths))
        }
    }

    /// Write a default `config.toml` into `paths.config_dir` and return it.
    ///
    /// Creates the directory if it does not exist. Safe to call repeatedly —
    /// subsequent calls overwrite the file with the same content.
    pub fn write_default(paths: &CodexPaths) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&paths.config_dir).with_context(|| {
            format!("Failed to create config dir: {}", paths.config_dir.display())
        })?;
        let config = Self::default_for_dirs(paths);
        let content = toml::to_string_pretty(&config)
            .context("Failed to serialize default config to TOML")?;
        let dest = paths.config_dir.join("config.toml");
        std::fs::write(&dest, &content)
            .with_context(|| format!("Failed to write config to {}", dest.display()))?;
        Ok(config)
    }

    /// Build a default `AppConfig` with paths derived from the provided
    /// `CodexPaths`. No reference to the working directory.
    pub fn default_for_dirs(paths: &CodexPaths) -> Self {
        let mut cfg = AppConfig::default();
        cfg.database.path = paths
            .data_dir
            .join("codex.db")
            .to_string_lossy()
            .into_owned();
        cfg.vault.base_dir = paths
            .default_vault_dir
            .to_string_lossy()
            .into_owned();
        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.database.path, "./codex.db");
        assert!(config.vault.index_exclusions.contains(&".git".to_string()));
        assert_eq!(config.vault.base_dir, "./vaults");
        assert!(!config.auth.enabled);
        assert_eq!(config.auth.provider, "password");
        assert_eq!(config.auth.access_token_ttl, 3600);
        assert_eq!(config.auth.refresh_token_ttl, 604800);
        assert_eq!(config.sync.change_log_retention_days, 7);
        assert_eq!(
            config.cors.allowed_origins,
            vec!["http://localhost:5173".to_string()]
        );
    }

    #[test]
    fn test_exclusions_default() {
        let exclusions = default_exclusions();
        assert!(exclusions.contains(&".git".to_string()));
        assert!(exclusions.contains(&".obsidian".to_string()));
        assert!(exclusions.contains(&".trash".to_string()));
    }

    // ── load_from_file ────────────────────────────────────────────────────

    #[test]
    fn test_load_from_file_missing_file_returns_error() {
        let result = AppConfig::load_from_file("/nonexistent/path/config.toml".into());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Config file not found"), "unexpected: {msg}");
    }

    #[test]
    fn test_load_from_file_malformed_toml_returns_error() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("bad.toml");
        std::fs::write(&path, "this is [[[not valid toml").unwrap();
        let result = AppConfig::load_from_file(path.clone());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains(path.display().to_string().as_str())
                || msg.contains("Failed"),
            "unexpected: {msg}"
        );
    }

    #[test]
    fn test_load_from_file_valid_file_overrides_defaults() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("config.toml");
        std::fs::write(&path, "[server]\nport = 9090\n").unwrap();
        let config = AppConfig::load_from_file(path).unwrap();
        assert_eq!(config.server.port, 9090);
        // Unspecified fields keep defaults
        assert_eq!(config.server.host, "127.0.0.1");
    }

    #[test]
    fn test_load_from_file_omitted_fields_use_defaults() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("config.toml");
        // Empty file — all defaults apply
        std::fs::write(&path, "").unwrap();
        let config = AppConfig::load_from_file(path).unwrap();
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.host, "127.0.0.1");
    }

    // ── load_from_dirs ────────────────────────────────────────────────────

    fn make_paths(temp: &TempDir) -> CodexPaths {
        CodexPaths {
            config_dir: temp.path().join("config"),
            data_dir: temp.path().join("data"),
            cache_dir: temp.path().join("cache"),
            default_vault_dir: temp.path().join("Documents/Codex"),
        }
    }

    #[test]
    fn test_load_from_dirs_no_config_file_returns_defaults() {
        let temp = TempDir::new().unwrap();
        let paths = make_paths(&temp);
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        // No config.toml — should succeed with default_for_dirs values
        let config = AppConfig::load_from_dirs(&paths).unwrap();
        assert!(config.database.path.contains("codex.db"));
        assert!(
            config.vault.base_dir.contains("Codex"),
            "base_dir should reference default vault dir"
        );
    }

    #[test]
    fn test_load_from_dirs_existing_file_is_read() {
        let temp = TempDir::new().unwrap();
        let paths = make_paths(&temp);
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        let config_path = paths.config_dir.join("config.toml");
        std::fs::write(&config_path, "[server]\nport = 7777\n").unwrap();
        let config = AppConfig::load_from_dirs(&paths).unwrap();
        assert_eq!(config.server.port, 7777);
    }

    // ── write_default ─────────────────────────────────────────────────────

    #[test]
    fn test_write_default_creates_file_at_correct_path() {
        let temp = TempDir::new().unwrap();
        let paths = make_paths(&temp);
        // config_dir does not yet exist
        AppConfig::write_default(&paths).unwrap();
        assert!(paths.config_dir.join("config.toml").exists());
    }

    #[test]
    fn test_write_default_content_round_trips() {
        let temp = TempDir::new().unwrap();
        let paths = make_paths(&temp);
        let written = AppConfig::write_default(&paths).unwrap();
        // Read back and compare key field
        let read_back = AppConfig::load_from_file(paths.config_dir.join("config.toml")).unwrap();
        assert_eq!(written.server.port, read_back.server.port);
        assert_eq!(written.database.path, read_back.database.path);
    }

    #[test]
    fn test_write_default_is_idempotent() {
        let temp = TempDir::new().unwrap();
        let paths = make_paths(&temp);
        AppConfig::write_default(&paths).unwrap();
        // Second call overwrites — should not error
        AppConfig::write_default(&paths).unwrap();
    }

    // ── default_for_dirs ──────────────────────────────────────────────────

    #[test]
    fn test_default_for_dirs_paths_resolve_relative_to_codex_paths() {
        let temp = TempDir::new().unwrap();
        let paths = make_paths(&temp);
        let config = AppConfig::default_for_dirs(&paths);
        assert!(
            config.database.path.contains("codex.db"),
            "database path should include codex.db"
        );
        assert!(
            Path::new(&config.database.path).is_absolute(),
            "database path should be absolute"
        );
        assert!(
            Path::new(&config.vault.base_dir).is_absolute(),
            "vault base_dir should be absolute"
        );
    }

    #[test]
    fn test_default_for_dirs_does_not_reference_working_directory() {
        let temp = TempDir::new().unwrap();
        let paths = make_paths(&temp);
        let config = AppConfig::default_for_dirs(&paths);
        assert!(
            !config.database.path.starts_with("./"),
            "database.path must not start with ./: {}",
            config.database.path
        );
        assert!(
            !config.vault.base_dir.starts_with("./"),
            "vault.base_dir must not start with ./: {}",
            config.vault.base_dir
        );
    }

    #[test]
    fn vault_config_document_format_defaults_to_markdown() {
        let json = r#"{}"#;
        let cfg: VaultConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.document_format, "markdown");
    }

    #[test]
    fn vault_config_document_format_round_trips() {
        let json = r#"{"document_format":"markdown"}"#;
        let cfg: VaultConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.document_format, "markdown");
        let serialized = serde_json::to_string(&cfg).unwrap();
        assert!(serialized.contains("\"document_format\":\"markdown\""));
    }
}
