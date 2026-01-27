use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plugin manifest format - defines plugin metadata and requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Unique identifier for the plugin (e.g., "com.example.myplugin")
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Semantic version (e.g., "1.0.0")
    pub version: String,

    /// Short description of plugin functionality
    pub description: Option<String>,

    /// Plugin author information
    pub author: Option<String>,

    /// License identifier (e.g., "MIT", "Apache-2.0")
    pub license: Option<String>,

    /// Entry point file (e.g., "main.js" for JS plugins, "plugin.wasm" for WASM)
    pub main: String,

    /// Plugin type: "javascript" or "wasm"
    #[serde(default = "default_plugin_type")]
    pub plugin_type: PluginType,

    /// CSS files to load
    #[serde(default)]
    pub styles: Vec<String>,

    /// Minimum required host version
    pub min_host_version: Option<String>,

    /// Plugin dependencies (plugin_id -> version requirement)
    #[serde(default)]
    pub dependencies: HashMap<String, String>,

    /// Required capabilities/permissions
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,

    /// Lifecycle hooks the plugin implements
    #[serde(default)]
    pub hooks: Vec<PluginHook>,

    /// Configuration schema (JSON Schema)
    pub config_schema: Option<serde_json::Value>,
}

fn default_plugin_type() -> PluginType {
    PluginType::JavaScript
}

/// Plugin execution type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    JavaScript,
    Wasm,
}

/// Plugin capabilities - what the plugin is allowed to do
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapability {
    /// Read files from vault
    ReadFiles,

    /// Write/modify files in vault
    WriteFiles,

    /// Delete files from vault
    DeleteFiles,

    /// Access vault metadata
    VaultMetadata,

    /// Make network requests
    Network,

    /// Access local storage/cache
    Storage,

    /// Modify UI (add ribbons, status bar items, etc.)
    ModifyUI,

    /// Register commands
    Commands,

    /// Access editor content
    EditorAccess,

    /// Execute system commands (highly restricted)
    SystemExec,
}

/// Plugin lifecycle hooks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PluginHook {
    /// Called when plugin is loaded
    OnLoad,

    /// Called when plugin is unloaded
    OnUnload,

    /// Called when a file is opened
    OnFileOpen,

    /// Called when a file is saved
    OnFileSave,

    /// Called when a file is created
    OnFileCreate,

    /// Called when a file is deleted
    OnFileDelete,

    /// Called when a file is renamed
    OnFileRename,

    /// Called when vault is switched
    OnVaultSwitch,

    /// Called when editor content changes
    OnEditorChange,

    /// Called on application startup
    OnStartup,

    /// Called on application shutdown
    OnShutdown,
}

/// Runtime plugin state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    /// Plugin manifest
    pub manifest: PluginManifest,

    /// Absolute path to plugin directory
    pub path: String,

    /// Whether plugin is currently enabled
    pub enabled: bool,

    /// Plugin load state
    pub state: PluginState,

    /// User configuration for this plugin
    #[serde(default)]
    pub config: serde_json::Value,

    /// Last error message (if any)
    pub last_error: Option<String>,
}

/// Plugin runtime state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PluginState {
    /// Plugin is not loaded
    Unloaded,

    /// Plugin is currently loading
    Loading,

    /// Plugin is loaded and active
    Loaded,

    /// Plugin failed to load
    Failed,

    /// Plugin is disabled by user
    Disabled,
}

impl Default for PluginState {
    fn default() -> Self {
        PluginState::Unloaded
    }
}

/// Plugin API context - passed to plugins for interaction with host
#[derive(Debug, Clone)]
pub struct PluginContext {
    /// Plugin ID
    pub plugin_id: String,

    /// Current vault ID (if any)
    pub vault_id: Option<String>,

    /// Granted capabilities
    pub capabilities: Vec<PluginCapability>,
}
