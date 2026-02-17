use crate::error::AppResult;
use crate::services::plugin_api::Event;
use async_trait::async_trait;

pub mod python;
pub mod wasm;

/// Trait that all plugin runtimes (WASM, Python, etc.) must implement
#[async_trait]
pub trait PluginRuntime: Send + Sync {
    /// Initialize and load the plugin
    async fn load(&mut self) -> AppResult<()>;

    /// Unload the plugin and cleanup resources
    async fn unload(&mut self) -> AppResult<()>;

    /// Handle an event dispatched to the plugin
    async fn on_event(&self, event: &Event) -> AppResult<()>;
}
