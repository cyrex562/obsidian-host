use serde::{Deserialize, Serialize};

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
    pub storage: StorageConfig,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_auth_enabled")]
    pub enabled: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_storage_backend")]
    pub backend: String,
    #[serde(default)]
    pub s3: S3StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct S3StorageConfig {
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub bucket: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub access_key: Option<String>,
    #[serde(default)]
    pub secret_key: Option<String>,
    #[serde(default = "default_s3_path_style")]
    pub path_style: bool,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_db_path() -> String {
    "./obsidian-host.db".to_string()
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

fn default_storage_backend() -> String {
    "local".to_string()
}

fn default_s3_path_style() -> bool {
    true
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
            },
            auth: AuthConfig {
                enabled: default_auth_enabled(),
                provider: default_auth_provider(),
                jwt_secret: default_jwt_secret(),
                access_token_ttl: default_access_token_ttl(),
                refresh_token_ttl: default_refresh_token_ttl(),
                bootstrap_admin_username: None,
                bootstrap_admin_password: None,
            },
            sync: SyncConfig {
                change_log_retention_days: default_change_log_retention_days(),
            },
            cors: CorsConfig {
                allowed_origins: default_cors_allowed_origins(),
            },
            storage: StorageConfig {
                backend: default_storage_backend(),
                s3: S3StorageConfig::default(),
            },
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

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: default_storage_backend(),
            s3: S3StorageConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self, config::ConfigError> {
        let config = config::Config::builder()
            // Start with defaults
            .add_source(config::File::from_str(
                &serde_json::to_string(&AppConfig::default()).unwrap(),
                config::FileFormat::Json,
            ))
            // Merge with local config file if exists
            .add_source(config::File::with_name("config").required(false))
            // Merge with environment variables
            .add_source(config::Environment::with_prefix("OBSIDIAN").separator("__"))
            .build()?;

        config.try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.database.path, "./obsidian-host.db");
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
        assert_eq!(config.storage.backend, "local");
    }

    #[test]
    fn test_exclusions_default() {
        let exclusions = default_exclusions();
        assert!(exclusions.contains(&".git".to_string()));
        assert!(exclusions.contains(&".obsidian".to_string()));
        assert!(exclusions.contains(&".trash".to_string()));
    }
}
