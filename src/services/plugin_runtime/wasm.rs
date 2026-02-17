use crate::error::{AppError, AppResult};
use crate::services::plugin_api::{Event, PluginApi};
use crate::models::plugin::PluginManifest;
use crate::services::plugin_runtime::PluginRuntime;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use wasmtime::{Config, Engine, Instance, Linker, Module, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};

struct PluginState {
    wasi: WasiCtx,
    api: PluginApi,
}


// Wrapper to hold Store and Instance together protected by Mutex
struct WasmRuntimeState {
    store: Store<PluginState>,
    instance: Instance,
}

pub struct WasmPluginRunner {
    api: PluginApi,
    plugin_dir: PathBuf,
    manifest: PluginManifest,
    runtime: Option<Arc<Mutex<WasmRuntimeState>>>,
}

impl WasmPluginRunner {
    pub fn new(api: PluginApi, plugin_dir: PathBuf, manifest: PluginManifest) -> Self {
        Self { 
            api, 
            plugin_dir, 
            manifest,
            runtime: None,
        }
    }
}

#[async_trait]
impl PluginRuntime for WasmPluginRunner {
    async fn load(&mut self) -> AppResult<()> {
        let mut config = Config::new();
        config.async_support(true);
        let engine = Engine::new(&config).map_err(|e| AppError::InternalError(e.to_string()))?;

        let wasi = WasiCtxBuilder::new()
            .inherit_stdio()
            .build();

        let plugin_state = PluginState {
            wasi,
            api: self.api.clone(),
        };

        let mut store = Store::new(&engine, plugin_state);
        let mut linker = Linker::new(&engine);
        
        // Add WASI to linker (Preview 1)
        wasmtime_wasi::add_to_linker(&mut linker, |s: &mut PluginState| &mut s.wasi).map_err(|e: anyhow::Error| AppError::InternalError(e.to_string()))?;

        // TODO: Add host functions to linker here
        // linker.func_wrap("obsidian", "log", ...)?;

        let module_path = self.plugin_dir.join(&self.manifest.main);
        let module = Module::from_file(&engine, &module_path)
            .map_err(|e| AppError::InternalError(format!("Failed to load WASM module from {:?}: {}", module_path, e)))?;

        let instance = linker.instantiate_async(&mut store, &module)
            .await
            .map_err(|e| AppError::InternalError(format!("Failed to instantiate WASM module: {}", e)))?;

        // detailed start/load logic:
        // Check for `on_load` export and call it if present
        if let Ok(on_load) = instance.get_typed_func::<(), ()>(&mut store, "on_load") {
            on_load.call_async(&mut store, ()).await
                .map_err(|e| AppError::InternalError(format!("Failed to call on_load: {}", e)))?;
        } else if let Ok(start) = instance.get_typed_func::<(), ()>(&mut store, "_start") {
             // _start is usually for CLI / WASI command. 
             // If it's a reactor, we expect explicit exports.
             // But if specific hooks are missing, maybe run _start?
             // Calling _start might terminate the module though.
             // We'll skip _start for now unless it's explicitly desired.
        }

        self.runtime = Some(Arc::new(Mutex::new(WasmRuntimeState {
            store,
            instance,
        })));

        Ok(())
    }

    async fn unload(&mut self) -> AppResult<()> {
        // Drop the runtime state
        self.runtime = None;
        Ok(())
    }

    async fn on_event(&self, event: &Event) -> AppResult<()> {
        if let Some(runtime) = &self.runtime {
             let mut state = runtime.lock().await;
             // Look for export "on_event" or specific handler?
             // Simple approach: "on_event" taking JSON string ptr/len?
             // For now, doing nothing.
        }
        Ok(())
    }
}
