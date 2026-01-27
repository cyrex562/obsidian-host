use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub vault: VaultConfig,
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
        assert_eq!(config.server.host, "127.0.0.1");
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
