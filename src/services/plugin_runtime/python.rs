use crate::error::{AppError, AppResult};
use crate::services::plugin_api::{Event, PluginApi};
use crate::models::plugin::PluginManifest;
use crate::services::PluginRuntime;
use async_trait::async_trait;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyAny};
use pyo3::PyObject;
use std::path::PathBuf;

#[pyclass]
#[derive(Clone)]
pub struct PyPluginApi {
    inner: PluginApi,
}

#[pymethods]
impl PyPluginApi {
    fn get_plugin_id(&self) -> String {
        self.inner.get_context().plugin_id.clone()
    }

    fn http_get(&self, url: String) -> PyResult<String> {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            self.inner.http_get(&url).await.map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    fn http_post(&self, url: String, body: String) -> PyResult<String> {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            self.inner.http_post(&url, body).await.map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    fn log(&self, message: String) {
        tracing::info!("[Plugin: {}] {}", self.get_plugin_id(), message);
    }

    fn show_notice(&self, message: String, duration: Option<u32>) -> PyResult<()> {
        self.inner.show_notice_blocking(&message, duration)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }
}

pub struct PythonPluginRunner {
    api: PluginApi,
    plugin_dir: PathBuf,
    manifest: PluginManifest,
    module: Option<PyObject>,
}

impl PythonPluginRunner {
    pub fn new(api: PluginApi, plugin_dir: PathBuf, manifest: PluginManifest) -> Self {
        Self {
            api,
            plugin_dir,
            manifest,
            module: None,
        }
    }
}

// Helper functions to isolate pyo3 from async-trait macro
fn load_python_module(plugin_dir: PathBuf, main_module: String, api: PluginApi) -> AppResult<PyObject> {
    Python::with_gil(|py| {
         // Add plugin dir to sys.path
        let syspath: &PyList = py.import("sys")?
            .getattr("path")?
            .downcast()
            .map_err(|_| AppError::InternalError("Failed to downcast sys.path".to_string()))?;
        syspath.insert(0, plugin_dir.to_string_lossy())?;

        // Import module
        let module = PyModule::import(py, main_module.as_str())
            .map_err(|e| AppError::InternalError(format!("Failed to import python module {}: {}", main_module, e)))?;

        // Call on_load if exists
        if module.hasattr("on_load")? {
            let py_api = PyPluginApi { inner: api };
            // Wrap in PyCell/PyObject
            let py_api_obj = Py::new(py, py_api)?;
            
            // Call on_load(api)
            let _ = module.getattr("on_load")?.call1((py_api_obj,))?;
        }

        Ok(module.into())
    })
}

fn unload_python_module(module: &PyObject) -> AppResult<()> {
    Python::with_gil(|py| {
        let module = module.as_ref(py);
        if module.hasattr("on_unload")? {
            let _ = module.getattr("on_unload")?.call0()?;
        }
        Ok(())
    })
}

fn dispatch_python_event(module: &PyObject, event: &Event) -> AppResult<()> {
    let event_json = serde_json::to_string(event).unwrap_or_default();
    Python::with_gil(|py| {
        let module = module.as_ref(py);
        if module.hasattr("on_event")? {
            // Pass event as JSON string
            let _ = module.getattr("on_event")?.call1((event_json,))?;
        }
        Ok(())
    })
}

#[async_trait]
impl PluginRuntime for PythonPluginRunner {
    async fn load(&mut self) -> AppResult<()> {
        let plugin_dir = self.plugin_dir.clone();
        let main_module = self.manifest.main.replace(".py", "");
        let api = self.api.clone();
        
        // Load module in blocking task to avoid blocking async runtime with GIL
        // Use explicit types to help inference if needed
        let module = tokio::task::spawn_blocking(move || {
            load_python_module(plugin_dir, main_module, api)
        }).await.map_err(|e: tokio::task::JoinError| AppError::InternalError(e.to_string()))??;
        
        self.module = Some(module);
        Ok(())
    }

    async fn unload(&mut self) -> AppResult<()> {
        let module_opt = self.module.clone();

        if let Some(module) = module_opt {
             tokio::task::spawn_blocking(move || {
                unload_python_module(&module)
            }).await.map_err(|e: tokio::task::JoinError| AppError::InternalError(e.to_string()))??;
        }
        self.module = None;
        Ok(())
    }

    async fn on_event(&self, event: &Event) -> AppResult<()> {
        let module_opt = self.module.clone();
        if let Some(module) = module_opt {
            let event = event.clone();
            tokio::task::spawn_blocking(move || {
                dispatch_python_event(&module, &event)
            }).await.map_err(|e: tokio::task::JoinError| AppError::InternalError(e.to_string()))??;
        }
        Ok(())
    }
}
