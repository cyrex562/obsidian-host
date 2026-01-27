/// Plugin API - Interface for plugins to interact with the host application
use crate::error::{AppError, AppResult};
use crate::models::plugin::{PluginCapability, PluginContext};
use crate::services::FileService;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Plugin API provides controlled access to host functionality
pub struct PluginApi {
    context: PluginContext,
    event_bus: Arc<RwLock<EventBus>>,
    storage: Arc<RwLock<PluginStorage>>,
}

impl PluginApi {
    pub fn new(
        context: PluginContext,
        event_bus: Arc<RwLock<EventBus>>,
        storage: Arc<RwLock<PluginStorage>>,
    ) -> Self {
        Self {
            context,
            event_bus,
            storage,
        }
    }

    /// Check if plugin has required capability
    fn check_capability(&self, capability: &PluginCapability) -> AppResult<()> {
        if !self.context.capabilities.contains(capability) {
            return Err(AppError::Forbidden(format!(
                "Plugin {} does not have {:?} capability",
                self.context.plugin_id, capability
            )));
        }
        Ok(())
    }

    // File System Operations

    /// Read file content from vault
    pub async fn read_file(&self, vault_path: &str, file_path: &str) -> AppResult<String> {
        self.check_capability(&PluginCapability::ReadFiles)?;

        let content = FileService::read_file(vault_path, file_path)?;
        Ok(content.content)
    }

    /// Write file content to vault
    pub async fn write_file(
        &self,
        vault_path: &str,
        file_path: &str,
        content: String,
    ) -> AppResult<()> {
        self.check_capability(&PluginCapability::WriteFiles)?;

        FileService::write_file(vault_path, file_path, &content, None, None)?;
        Ok(())
    }

    /// Delete file from vault
    pub async fn delete_file(&self, vault_path: &str, file_path: &str) -> AppResult<()> {
        self.check_capability(&PluginCapability::DeleteFiles)?;

        FileService::delete_file(vault_path, file_path)?;
        Ok(())
    }

    /// List files in vault
    pub async fn list_files(
        &self,
        vault_path: &str,
        pattern: Option<&str>,
    ) -> AppResult<Vec<String>> {
        self.check_capability(&PluginCapability::ReadFiles)?;

        let tree = FileService::get_file_tree(vault_path)?;
        let mut files = Vec::new();
        collect_files(&tree, &mut files);

        if let Some(pattern) = pattern {
            files.retain(|f| f.contains(pattern));
        }

        Ok(files)
    }

    // Event System

    /// Subscribe to an event
    pub async fn on_event<F>(&self, event_type: EventType, callback: F) -> AppResult<String>
    where
        F: Fn(Event) + Send + Sync + 'static,
    {
        let mut bus = self.event_bus.write().await;
        let subscription_id = bus.subscribe(event_type, Box::new(callback));
        Ok(subscription_id)
    }

    /// Emit an event
    pub async fn emit_event(&self, event: Event) -> AppResult<()> {
        let bus = self.event_bus.read().await;
        bus.emit(event).await;
        Ok(())
    }

    /// Unsubscribe from an event
    pub async fn off_event(&self, subscription_id: &str) -> AppResult<()> {
        let mut bus = self.event_bus.write().await;
        bus.unsubscribe(subscription_id);
        Ok(())
    }

    // Storage Operations

    /// Get plugin-specific storage value
    pub async fn storage_get(&self, key: &str) -> AppResult<Option<serde_json::Value>> {
        self.check_capability(&PluginCapability::Storage)?;

        let storage = self.storage.read().await;
        Ok(storage.get(&self.context.plugin_id, key))
    }

    /// Set plugin-specific storage value
    pub async fn storage_set(&self, key: &str, value: serde_json::Value) -> AppResult<()> {
        self.check_capability(&PluginCapability::Storage)?;

        let mut storage = self.storage.write().await;
        storage.set(&self.context.plugin_id, key, value);
        Ok(())
    }

    /// Delete plugin-specific storage value
    pub async fn storage_delete(&self, key: &str) -> AppResult<()> {
        self.check_capability(&PluginCapability::Storage)?;

        let mut storage = self.storage.write().await;
        storage.delete(&self.context.plugin_id, key);
        Ok(())
    }

    /// Clear all plugin-specific storage
    pub async fn storage_clear(&self) -> AppResult<()> {
        self.check_capability(&PluginCapability::Storage)?;

        let mut storage = self.storage.write().await;
        storage.clear(&self.context.plugin_id);
        Ok(())
    }

    // Markdown Utilities

    /// Parse markdown to HTML
    pub async fn parse_markdown(&self, markdown: &str) -> AppResult<String> {
        use pulldown_cmark::{html, Parser};

        let parser = Parser::new(markdown);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        Ok(html_output)
    }

    /// Extract frontmatter from markdown
    pub async fn extract_frontmatter(&self, content: &str) -> AppResult<Option<serde_json::Value>> {
        use crate::services::frontmatter_service;

        let (frontmatter, _) = frontmatter_service::parse_frontmatter(content)?;
        Ok(frontmatter)
    }

    // Network Operations (if capability granted)

    /// Make HTTP GET request
    pub async fn http_get(&self, url: &str) -> AppResult<String> {
        self.check_capability(&PluginCapability::Network)?;

        // TODO: Implement HTTP client
        Err(AppError::InternalError(
            "HTTP client not yet implemented".to_string(),
        ))
    }

    /// Make HTTP POST request
    pub async fn http_post(&self, url: &str, body: String) -> AppResult<String> {
        self.check_capability(&PluginCapability::Network)?;

        // TODO: Implement HTTP client
        Err(AppError::InternalError(
            "HTTP client not yet implemented".to_string(),
        ))
    }

    // UI Extension Points

    /// Register a command
    pub async fn register_command(&self, command: Command) -> AppResult<String> {
        self.check_capability(&PluginCapability::Commands)?;

        let mut bus = self.event_bus.write().await;
        let command_id = format!("{}:{}", self.context.plugin_id, command.id);
        bus.register_command(command_id.clone(), command);
        Ok(command_id)
    }

    /// Show notification to user
    pub async fn show_notice(&self, message: &str, duration_ms: Option<u32>) -> AppResult<()> {
        self.check_capability(&PluginCapability::ModifyUI)?;

        self.emit_event(Event {
            event_type: EventType::ShowNotice,
            data: serde_json::json!({
                "message": message,
                "duration": duration_ms.unwrap_or(3000)
            }),
        })
        .await
    }

    // Plugin-to-Plugin Communication

    /// Send message to another plugin
    pub async fn send_message(
        &self,
        target_plugin: &str,
        message: serde_json::Value,
    ) -> AppResult<()> {
        self.emit_event(Event {
            event_type: EventType::PluginMessage,
            data: serde_json::json!({
                "from": self.context.plugin_id,
                "to": target_plugin,
                "message": message
            }),
        })
        .await
    }

    /// Get plugin context
    pub fn get_context(&self) -> &PluginContext {
        &self.context
    }
}

// Helper function to collect files from tree
fn collect_files(nodes: &[crate::models::FileNode], files: &mut Vec<String>) {
    for node in nodes {
        if !node.is_directory {
            files.push(node.path.clone());
        }
        if let Some(ref children) = node.children {
            collect_files(children, files);
        }
    }
}

/// Event system for plugin communication
#[derive(Debug, Clone)]
pub struct Event {
    pub event_type: EventType,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    FileOpen,
    FileSave,
    FileCreate,
    FileDelete,
    FileRename,
    VaultSwitch,
    EditorChange,
    ShowNotice,
    PluginMessage,
    CommandExecute,
}

pub struct EventBus {
    subscribers: HashMap<EventType, Vec<(String, Box<dyn Fn(Event) + Send + Sync>)>>,
    commands: HashMap<String, Command>,
    next_id: usize,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: HashMap::new(),
            commands: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn subscribe<F>(&mut self, event_type: EventType, callback: Box<F>) -> String
    where
        F: Fn(Event) + Send + Sync + 'static,
    {
        let id = format!("sub_{}", self.next_id);
        self.next_id += 1;

        self.subscribers
            .entry(event_type)
            .or_insert_with(Vec::new)
            .push((id.clone(), callback));

        id
    }

    pub fn unsubscribe(&mut self, subscription_id: &str) {
        for subscribers in self.subscribers.values_mut() {
            subscribers.retain(|(id, _)| id != subscription_id);
        }
    }

    pub async fn emit(&self, event: Event) {
        if let Some(subscribers) = self.subscribers.get(&event.event_type) {
            for (_, callback) in subscribers {
                callback(event.clone());
            }
        }
    }

    pub fn register_command(&mut self, command_id: String, command: Command) {
        self.commands.insert(command_id, command);
    }

    pub fn get_command(&self, command_id: &str) -> Option<&Command> {
        self.commands.get(command_id)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Command registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub hotkey: Option<String>,
}

/// Plugin storage system
pub struct PluginStorage {
    data: HashMap<String, HashMap<String, serde_json::Value>>,
}

impl PluginStorage {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn get(&self, plugin_id: &str, key: &str) -> Option<serde_json::Value> {
        self.data
            .get(plugin_id)
            .and_then(|plugin_data| plugin_data.get(key))
            .cloned()
    }

    pub fn set(&mut self, plugin_id: &str, key: &str, value: serde_json::Value) {
        self.data
            .entry(plugin_id.to_string())
            .or_insert_with(HashMap::new)
            .insert(key.to_string(), value);
    }

    pub fn delete(&mut self, plugin_id: &str, key: &str) {
        if let Some(plugin_data) = self.data.get_mut(plugin_id) {
            plugin_data.remove(key);
        }
    }

    pub fn clear(&mut self, plugin_id: &str) {
        self.data.remove(plugin_id);
    }

    pub fn get_all(&self, plugin_id: &str) -> Option<&HashMap<String, serde_json::Value>> {
        self.data.get(plugin_id)
    }
}

impl Default for PluginStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_storage() {
        let mut storage = PluginStorage::new();

        storage.set("plugin1", "key1", serde_json::json!("value1"));
        assert_eq!(
            storage.get("plugin1", "key1"),
            Some(serde_json::json!("value1"))
        );

        storage.delete("plugin1", "key1");
        assert_eq!(storage.get("plugin1", "key1"), None);
    }

    #[test]
    fn test_event_bus() {
        let mut bus = EventBus::new();

        let sub_id = bus.subscribe(
            EventType::FileOpen,
            Box::new(|_event| {
                // Callback
            }),
        );

        assert!(sub_id.starts_with("sub_"));

        bus.unsubscribe(&sub_id);
    }
}
