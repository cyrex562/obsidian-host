use crate::models::plugin::{Plugin, PluginCapability, PluginManifest, PluginState};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

#[derive(Debug, Serialize, Deserialize, Default)]
struct PluginConfig {
    enabled_plugins: HashSet<String>,
}

pub struct PluginService {
    plugins_dir: PathBuf,
    config_path: PathBuf,
    plugins: HashMap<String, Plugin>,
    load_order: Vec<String>,
    config: PluginConfig,
}

impl PluginService {
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        let plugins_dir = plugins_dir.into();
        let config_path = resolve_plugin_config_path(&plugins_dir);
        let config = Self::load_config(&config_path).unwrap_or_default();

        Self {
            plugins_dir,
            config_path,
            plugins: HashMap::new(),
            load_order: Vec::new(),
            config,
        }
    }

    /// Load plugin configuration from disk
    fn load_config(config_path: &Path) -> Result<PluginConfig, String> {
        if !config_path.exists() {
            return Ok(PluginConfig::default());
        }

        let content =
            fs::read_to_string(config_path).map_err(|e| format!("Failed to read config: {}", e))?;

        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))
    }

    /// Save plugin configuration to disk
    fn save_config(&self) -> Result<(), String> {
        let content = serde_json::to_string_pretty(&self.config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        fs::write(&self.config_path, content)
            .map_err(|e| format!("Failed to write config: {}", e))?;

        Ok(())
    }

    /// Scan the plugins directory and discover all plugins
    pub fn discover_plugins(&mut self) -> Result<Vec<Plugin>, String> {
        info!("Discovering plugins in {:?}", self.plugins_dir);

        if !self.plugins_dir.exists() {
            warn!("Plugins directory does not exist: {:?}", self.plugins_dir);
            fs::create_dir_all(&self.plugins_dir)
                .map_err(|e| format!("Failed to create plugins directory: {}", e))?;
            return Ok(Vec::new());
        }

        let entries = fs::read_dir(&self.plugins_dir)
            .map_err(|e| format!("Failed to read plugins directory: {}", e))?;

        let is_first_run = self.config.enabled_plugins.is_empty();
        let mut discovered_ids = Vec::new();

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            if path.is_dir() {
                match self.load_plugin_manifest(&path) {
                    Ok(plugin) => {
                        let plugin_id = plugin.manifest.id.clone();
                        discovered_ids.push(plugin_id.clone());

                        // On first run, add all plugins to enabled set
                        if is_first_run {
                            self.config.enabled_plugins.insert(plugin_id.clone());
                        }

                        self.plugins.insert(plugin_id, plugin);
                    }
                    Err(e) => {
                        warn!("Failed to load plugin from {:?}: {}", path, e);
                    }
                }
            }
        }

        // Save config if this was first run
        if is_first_run && !discovered_ids.is_empty() {
            let _ = self.save_config();
        }

        info!("Discovered {} plugins", self.plugins.len());
        Ok(self.plugins.values().cloned().collect())
    }

    /// Load plugin manifest from directory
    fn load_plugin_manifest(&self, plugin_dir: &Path) -> Result<Plugin, String> {
        let manifest_path = plugin_dir.join("manifest.json");
        if !manifest_path.exists() {
            return Err("manifest.json not found".to_string());
        }

        let content = fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Failed to read manifest: {}", e))?;

        let manifest: PluginManifest = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse manifest: {}", e))?;

        // Validate manifest
        self.validate_manifest(&manifest)?;

        // Check if plugin is enabled (must be in the enabled set)
        let enabled = self.config.enabled_plugins.contains(&manifest.id);

        Ok(Plugin {
            manifest,
            path: plugin_dir.to_string_lossy().to_string(),
            enabled,
            state: if enabled {
                PluginState::Unloaded
            } else {
                PluginState::Disabled
            },
            config: serde_json::Value::Null,
            last_error: None,
        })
    }

    /// Validate plugin manifest
    fn validate_manifest(&self, manifest: &PluginManifest) -> Result<(), String> {
        if manifest.id.is_empty() {
            return Err("Plugin ID cannot be empty".to_string());
        }

        if manifest.name.is_empty() {
            return Err("Plugin name cannot be empty".to_string());
        }

        if manifest.main.is_empty() {
            return Err("Plugin main entry point cannot be empty".to_string());
        }

        // Validate version format (basic semver check)
        if !is_valid_semver(&manifest.version) {
            return Err(format!("Invalid version format: {}", manifest.version));
        }

        // Validate minimum host version if specified
        if let Some(ref min_version) = manifest.min_host_version {
            if !is_valid_semver(min_version) {
                return Err(format!("Invalid min_host_version format: {}", min_version));
            }

            // Check if current host version meets requirement
            let host_version = env!("CARGO_PKG_VERSION");
            if !version_satisfies(host_version, min_version) {
                return Err(format!(
                    "Plugin requires host version {} but current version is {}",
                    min_version, host_version
                ));
            }
        }

        Ok(())
    }

    /// Resolve plugin dependencies and determine load order
    pub fn resolve_dependencies(&mut self) -> Result<Vec<String>, String> {
        debug!("Resolving plugin dependencies");

        let mut load_order = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        for plugin_id in self.plugins.keys().cloned().collect::<Vec<_>>() {
            self.visit_plugin(&plugin_id, &mut visited, &mut visiting, &mut load_order)?;
        }

        self.load_order = load_order.clone();
        info!("Plugin load order: {:?}", self.load_order);
        Ok(load_order)
    }

    /// Visit plugin in dependency graph (DFS for topological sort)
    fn visit_plugin(
        &self,
        plugin_id: &str,
        visited: &mut HashSet<String>,
        visiting: &mut HashSet<String>,
        load_order: &mut Vec<String>,
    ) -> Result<(), String> {
        if visited.contains(plugin_id) {
            return Ok(());
        }

        if visiting.contains(plugin_id) {
            return Err(format!(
                "Circular dependency detected involving plugin: {}",
                plugin_id
            ));
        }

        visiting.insert(plugin_id.to_string());

        if let Some(plugin) = self.plugins.get(plugin_id) {
            // Visit dependencies first
            for (dep_id, version_req) in &plugin.manifest.dependencies {
                // Check if dependency exists
                if let Some(dep_plugin) = self.plugins.get(dep_id) {
                    // Verify version compatibility
                    if !version_satisfies(&dep_plugin.manifest.version, version_req) {
                        return Err(format!(
                            "Plugin {} requires {} version {}, but found {}",
                            plugin_id, dep_id, version_req, dep_plugin.manifest.version
                        ));
                    }

                    self.visit_plugin(dep_id, visited, visiting, load_order)?;
                } else {
                    return Err(format!(
                        "Plugin {} depends on {} which is not installed",
                        plugin_id, dep_id
                    ));
                }
            }
        }

        visiting.remove(plugin_id);
        visited.insert(plugin_id.to_string());
        load_order.push(plugin_id.to_string());

        Ok(())
    }

    /// Enable a plugin
    pub fn enable_plugin(&mut self, plugin_id: &str) -> Result<(), String> {
        let plugin = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| format!("Plugin not found: {}", plugin_id))?;

        if plugin.enabled {
            return Ok(());
        }

        plugin.enabled = true;
        plugin.state = PluginState::Unloaded;
        self.config.enabled_plugins.insert(plugin_id.to_string());
        self.save_config()?;
        info!("Enabled plugin: {}", plugin_id);
        Ok(())
    }

    /// Disable a plugin
    pub fn disable_plugin(&mut self, plugin_id: &str) -> Result<(), String> {
        let plugin = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| format!("Plugin not found: {}", plugin_id))?;

        if !plugin.enabled {
            return Ok(());
        }

        plugin.enabled = false;
        plugin.state = PluginState::Disabled;
        self.config.enabled_plugins.remove(plugin_id);
        self.save_config()?;
        info!("Disabled plugin: {}", plugin_id);
        Ok(())
    }

    /// Get all plugins
    pub fn get_plugins(&self) -> Vec<Plugin> {
        self.plugins.values().cloned().collect()
    }

    /// Get plugin by ID
    pub fn get_plugin(&self, plugin_id: &str) -> Option<&Plugin> {
        self.plugins.get(plugin_id)
    }

    /// Get enabled plugins in load order
    pub fn get_enabled_plugins(&self) -> Vec<Plugin> {
        self.load_order
            .iter()
            .filter_map(|id| self.plugins.get(id))
            .filter(|p| p.enabled)
            .cloned()
            .collect()
    }

    /// Update plugin state
    pub fn update_plugin_state(
        &mut self,
        plugin_id: &str,
        state: PluginState,
    ) -> Result<(), String> {
        let plugin = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| format!("Plugin not found: {}", plugin_id))?;

        plugin.state = state;
        Ok(())
    }

    /// Set plugin error
    pub fn set_plugin_error(&mut self, plugin_id: &str, error: String) -> Result<(), String> {
        let plugin = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| format!("Plugin not found: {}", plugin_id))?;

        plugin.last_error = Some(error);
        plugin.state = PluginState::Failed;
        Ok(())
    }

    /// Update plugin configuration, validating against the plugin's JSON Schema if one
    /// is declared in its manifest.
    pub fn update_plugin_config(
        &mut self,
        plugin_id: &str,
        config: serde_json::Value,
    ) -> Result<(), String> {
        let plugin = self
            .plugins
            .get_mut(plugin_id)
            .ok_or_else(|| format!("Plugin not found: {}", plugin_id))?;

        if let Some(schema) = &plugin.manifest.config_schema {
            let validator = jsonschema::validator_for(schema)
                .map_err(|e| format!("Invalid config schema for plugin {plugin_id}: {e}"))?;
            let errors: Vec<String> = validator
                .iter_errors(&config)
                .map(|e| e.to_string())
                .collect();
            if !errors.is_empty() {
                return Err(format!(
                    "Config validation failed for plugin {plugin_id}: {}",
                    errors.join("; ")
                ));
            }
        }

        plugin.config = config;
        Ok(())
    }

    /// Check if plugin has capability
    pub fn has_capability(&self, plugin_id: &str, capability: &PluginCapability) -> bool {
        self.plugins
            .get(plugin_id)
            .map(|p| p.manifest.capabilities.contains(capability))
            .unwrap_or(false)
    }

    /// Get plugin statistics
    pub fn get_stats(&self) -> PluginStats {
        let total = self.plugins.len();
        let enabled = self.plugins.values().filter(|p| p.enabled).count();
        let loaded = self
            .plugins
            .values()
            .filter(|p| p.state == PluginState::Loaded)
            .count();
        let failed = self
            .plugins
            .values()
            .filter(|p| p.state == PluginState::Failed)
            .count();

        PluginStats {
            total,
            enabled,
            loaded,
            failed,
        }
    }

    /// Legacy method for compatibility
    pub fn scan_plugins(&self) -> Vec<Plugin> {
        self.get_plugins()
    }
}

#[derive(Debug, Clone)]
pub struct PluginStats {
    pub total: usize,
    pub enabled: usize,
    pub loaded: usize,
    pub failed: usize,
}

pub fn resolve_plugins_dir() -> PathBuf {
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let bundled_plugins = exe_dir.join("plugins");
            if bundled_plugins.exists() {
                return bundled_plugins;
            }
        }
    }

    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("plugins")
}

fn resolve_plugin_config_path(plugins_dir: &Path) -> PathBuf {
    if let Ok(cwd) = env::current_dir() {
        let cwd_plugins = cwd.join("plugins");
        if cwd_plugins != plugins_dir {
            return cwd.join(".plugins_config.json");
        }
    }

    plugins_dir.join(".plugins_config.json")
}

/// Check if version string is valid semver
fn is_valid_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return false;
    }

    parts.iter().all(|p| p.parse::<u32>().is_ok())
}

/// Check if version satisfies requirement (simplified semver)
fn version_satisfies(version: &str, requirement: &str) -> bool {
    // Handle exact version
    if !requirement.starts_with('^')
        && !requirement.starts_with('~')
        && !requirement.starts_with(">=")
    {
        return version == requirement;
    }

    let req = requirement
        .trim_start_matches('^')
        .trim_start_matches('~')
        .trim_start_matches(">=");
    let version_parts: Vec<u32> = version.split('.').filter_map(|s| s.parse().ok()).collect();
    let req_parts: Vec<u32> = req.split('.').filter_map(|s| s.parse().ok()).collect();

    if version_parts.len() != 3 || req_parts.len() != 3 {
        return false;
    }

    if requirement.starts_with('^') {
        // Compatible with same major version
        version_parts[0] == req_parts[0]
            && (version_parts[1] > req_parts[1]
                || (version_parts[1] == req_parts[1] && version_parts[2] >= req_parts[2]))
    } else if requirement.starts_with('~') {
        // Compatible with same minor version
        version_parts[0] == req_parts[0]
            && version_parts[1] == req_parts[1]
            && version_parts[2] >= req_parts[2]
    } else if requirement.starts_with(">=") {
        // Greater than or equal
        version_parts >= req_parts
    } else {
        version == requirement
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_validation() {
        assert!(is_valid_semver("1.0.0"));
        assert!(is_valid_semver("0.1.0"));
        assert!(!is_valid_semver("1.0"));
        assert!(!is_valid_semver("invalid"));
    }

    #[test]
    fn test_version_satisfies() {
        assert!(version_satisfies("1.2.3", "1.2.3"));
        assert!(version_satisfies("1.5.0", "^1.0.0"));
        assert!(!version_satisfies("2.0.0", "^1.0.0"));
        assert!(version_satisfies("1.2.5", "~1.2.0"));
        assert!(!version_satisfies("1.3.0", "~1.2.0"));
    }

    fn make_plugin_with_schema(schema: Option<serde_json::Value>) -> (PluginService, String) {
        use crate::models::plugin::{PluginManifest, PluginState, PluginType};
        let plugin_id = "test.plugin".to_string();
        let manifest = PluginManifest {
            id: plugin_id.clone(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            license: None,
            main: "main.js".to_string(),
            plugin_type: PluginType::JavaScript,
            styles: vec![],
            min_host_version: None,
            dependencies: Default::default(),
            capabilities: vec![],
            hooks: vec![],
            config_schema: schema,
            entity_types: vec![],
            relation_types: vec![],
            labels: vec![],
        };
        let plugin = Plugin {
            manifest,
            path: "/tmp/test-plugin".to_string(),
            enabled: true,
            state: PluginState::Loaded,
            config: serde_json::Value::Null,
            last_error: None,
        };
        let dir = tempfile::TempDir::new().unwrap();
        let mut svc = PluginService::new(dir.path());
        svc.plugins.insert(plugin_id.clone(), plugin);
        (svc, plugin_id)
    }

    #[test]
    fn update_plugin_config_no_schema_accepts_anything() {
        let (mut svc, id) = make_plugin_with_schema(None);
        let result = svc.update_plugin_config(&id, serde_json::json!({"key": "value"}));
        assert!(result.is_ok());
    }

    #[test]
    fn update_plugin_config_valid_config_passes_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "level": { "type": "number" } },
            "required": ["level"]
        });
        let (mut svc, id) = make_plugin_with_schema(Some(schema));
        let result = svc.update_plugin_config(&id, serde_json::json!({"level": 3}));
        assert!(result.is_ok());
    }

    #[test]
    fn update_plugin_config_invalid_config_rejected_by_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "level": { "type": "number" } },
            "required": ["level"]
        });
        let (mut svc, id) = make_plugin_with_schema(Some(schema));
        // "level" is required but missing
        let result = svc.update_plugin_config(&id, serde_json::json!({"other": "field"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("validation failed"));
    }

    #[test]
    fn update_plugin_config_wrong_type_rejected() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "level": { "type": "number" } },
            "required": ["level"]
        });
        let (mut svc, id) = make_plugin_with_schema(Some(schema));
        // "level" should be a number, not a string
        let result = svc.update_plugin_config(&id, serde_json::json!({"level": "high"}));
        assert!(result.is_err());
    }
}
