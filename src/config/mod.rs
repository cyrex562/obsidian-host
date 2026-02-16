use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Whether authentication is enabled. When false, all requests are allowed without login.
    #[serde(default = "default_auth_enabled")]
    pub enabled: bool,

    /// Google OAuth client ID
    #[serde(default)]
    pub google_client_id: String,

    /// Google OAuth client secret
    #[serde(default)]
    pub google_client_secret: String,

    /// The external URL of this application (used for redirect URI construction)
    /// e.g., "http://localhost:8080" or "https://notes.example.com"
    #[serde(default = "default_external_url")]
    pub external_url: String,

    /// Session duration in hours (default: 168 = 7 days)
    #[serde(default = "default_session_hours")]
    pub session_duration_hours: i64,

    /// Secret key for signing session cookies (should be random, >= 32 chars)
    #[serde(default = "default_session_secret")]
    pub session_secret: String,

    /// Force Secure flag on cookies even when the app receives HTTP traffic.
    /// Enable this when behind a TLS-terminating proxy (e.g., Tailscale Funnel, nginx).
    #[serde(default)]
    pub force_secure_cookies: bool,
}

fn default_auth_enabled() -> bool {
    false
}

fn default_external_url() -> String {
    "http://localhost:8080".to_string()
}

fn default_session_hours() -> i64 {
    168 // 7 days
}

fn default_session_secret() -> String {
    // Generate a random secret if not configured (not stable across restarts)
    use rand::Rng;
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
    hex::encode(bytes)
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: default_auth_enabled(),
            google_client_id: String::new(),
            google_client_secret: String::new(),
            external_url: default_external_url(),
            session_duration_hours: default_session_hours(),
            session_secret: default_session_secret(),
            force_secure_cookies: false,
        }
    }
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
    #[serde(default = "default_exclusions")]
    pub index_exclusions: Vec<String>,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
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
                index_exclusions: default_exclusions(),
            },
            auth: AuthConfig::default(),
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
            index_exclusions: default_exclusions(),
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
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.database.path, "./obsidian-host.db");
        assert!(config.vault.index_exclusions.contains(&".git".to_string()));
    }

    #[test]
    fn test_exclusions_default() {
        let exclusions = default_exclusions();
        assert!(exclusions.contains(&".git".to_string()));
        assert!(exclusions.contains(&".obsidian".to_string()));
        assert!(exclusions.contains(&".trash".to_string()));
    }
}
