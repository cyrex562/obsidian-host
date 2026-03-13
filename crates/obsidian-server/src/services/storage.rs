use crate::config::StorageConfig;
use crate::error::{AppError, AppResult};
use crate::models::FileContent;
use crate::services::FileService;
use chrono::{DateTime, Utc};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

pub trait StorageBackend: Send + Sync {
    fn backend_name(&self) -> &'static str;
    fn read_raw(&self, vault_path: &str, file_path: &str) -> AppResult<Vec<u8>>;
    fn create_file(
        &self,
        vault_path: &str,
        file_path: &str,
        content: Option<&str>,
    ) -> AppResult<FileContent>;
    fn write_file(
        &self,
        vault_path: &str,
        file_path: &str,
        content: &str,
        last_modified: Option<DateTime<Utc>>,
        frontmatter: Option<&serde_json::Value>,
    ) -> AppResult<FileContent>;
    fn create_upload_session_temp(&self, vault_path: &str, session_id: &str) -> AppResult<()>;
    fn append_upload_chunk(
        &self,
        vault_path: &str,
        session_id: &str,
        bytes: &[u8],
    ) -> AppResult<u64>;
    fn get_upload_session_size(&self, vault_path: &str, session_id: &str) -> AppResult<u64>;
    fn finalize_upload_session(
        &self,
        vault_path: &str,
        session_id: &str,
        target_dir: &str,
        filename: &str,
    ) -> AppResult<String>;
    fn delete_upload_session_temp(&self, vault_path: &str, session_id: &str) -> AppResult<()>;
}

#[derive(Default)]
pub struct LocalFsStorageBackend;

impl StorageBackend for LocalFsStorageBackend {
    fn backend_name(&self) -> &'static str {
        "local"
    }

    fn read_raw(&self, vault_path: &str, file_path: &str) -> AppResult<Vec<u8>> {
        FileService::read_raw_file(vault_path, file_path)
    }

    fn create_file(
        &self,
        vault_path: &str,
        file_path: &str,
        content: Option<&str>,
    ) -> AppResult<FileContent> {
        FileService::create_file(vault_path, file_path, content)
    }

    fn write_file(
        &self,
        vault_path: &str,
        file_path: &str,
        content: &str,
        last_modified: Option<DateTime<Utc>>,
        frontmatter: Option<&serde_json::Value>,
    ) -> AppResult<FileContent> {
        FileService::write_file(vault_path, file_path, content, last_modified, frontmatter)
    }

    fn create_upload_session_temp(&self, vault_path: &str, session_id: &str) -> AppResult<()> {
        let upload_dir = Path::new(vault_path).join(".obsidian").join("uploads");
        std::fs::create_dir_all(&upload_dir)?;
        let temp_file_path = upload_dir.join(session_id);
        std::fs::File::create(temp_file_path)?;
        Ok(())
    }

    fn append_upload_chunk(
        &self,
        vault_path: &str,
        session_id: &str,
        bytes: &[u8],
    ) -> AppResult<u64> {
        let temp_file_path = upload_temp_file_path(vault_path, session_id);
        if !temp_file_path.exists() {
            return Err(AppError::NotFound("Upload session not found".to_string()));
        }

        let mut file = OpenOptions::new().append(true).open(&temp_file_path)?;
        file.write_all(bytes)?;
        Ok(file.metadata()?.len())
    }

    fn get_upload_session_size(&self, vault_path: &str, session_id: &str) -> AppResult<u64> {
        let temp_file_path = upload_temp_file_path(vault_path, session_id);
        if !temp_file_path.exists() {
            return Err(AppError::NotFound("Upload session not found".to_string()));
        }
        Ok(std::fs::metadata(temp_file_path)?.len())
    }

    fn finalize_upload_session(
        &self,
        vault_path: &str,
        session_id: &str,
        target_dir: &str,
        filename: &str,
    ) -> AppResult<String> {
        validate_upload_filename(filename)?;

        let temp_file_path = upload_temp_file_path(vault_path, session_id);
        if !temp_file_path.exists() {
            return Err(AppError::NotFound("Upload session not found".to_string()));
        }

        let safe_target_dir = if target_dir.is_empty() {
            Path::new(vault_path).to_path_buf()
        } else {
            FileService::resolve_path(vault_path, target_dir)?
        };

        if !safe_target_dir.exists() {
            std::fs::create_dir_all(&safe_target_dir)?;
        } else if !safe_target_dir.is_dir() {
            return Err(AppError::InvalidInput(
                "Target path is not a directory".to_string(),
            ));
        }

        let final_path = safe_target_dir.join(filename);
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if let Err(_) = std::fs::rename(&temp_file_path, &final_path) {
            std::fs::copy(&temp_file_path, &final_path)?;
            std::fs::remove_file(&temp_file_path)?;
        }

        let relative = final_path
            .strip_prefix(vault_path)
            .unwrap_or(&final_path)
            .to_string_lossy()
            .to_string();

        Ok(relative)
    }

    fn delete_upload_session_temp(&self, vault_path: &str, session_id: &str) -> AppResult<()> {
        let temp_file_path = upload_temp_file_path(vault_path, session_id);
        if temp_file_path.exists() {
            std::fs::remove_file(temp_file_path)?;
        }
        Ok(())
    }
}

pub struct S3StorageBackend;

impl StorageBackend for S3StorageBackend {
    fn backend_name(&self) -> &'static str {
        "s3"
    }

    fn read_raw(&self, _vault_path: &str, _file_path: &str) -> AppResult<Vec<u8>> {
        Err(AppError::InternalError(
            "S3 storage backend is not implemented yet. Use storage.backend = \"local\" for now."
                .to_string(),
        ))
    }

    fn create_file(
        &self,
        _vault_path: &str,
        _file_path: &str,
        _content: Option<&str>,
    ) -> AppResult<FileContent> {
        Err(AppError::InternalError(
            "S3 storage backend is not implemented yet. Use storage.backend = \"local\" for now."
                .to_string(),
        ))
    }

    fn write_file(
        &self,
        _vault_path: &str,
        _file_path: &str,
        _content: &str,
        _last_modified: Option<DateTime<Utc>>,
        _frontmatter: Option<&serde_json::Value>,
    ) -> AppResult<FileContent> {
        Err(AppError::InternalError(
            "S3 storage backend is not implemented yet. Use storage.backend = \"local\" for now."
                .to_string(),
        ))
    }

    fn create_upload_session_temp(&self, _vault_path: &str, _session_id: &str) -> AppResult<()> {
        Err(AppError::InternalError(
            "S3 storage backend is not implemented yet. Use storage.backend = \"local\" for now."
                .to_string(),
        ))
    }

    fn append_upload_chunk(
        &self,
        _vault_path: &str,
        _session_id: &str,
        _bytes: &[u8],
    ) -> AppResult<u64> {
        Err(AppError::InternalError(
            "S3 storage backend is not implemented yet. Use storage.backend = \"local\" for now."
                .to_string(),
        ))
    }

    fn get_upload_session_size(&self, _vault_path: &str, _session_id: &str) -> AppResult<u64> {
        Err(AppError::InternalError(
            "S3 storage backend is not implemented yet. Use storage.backend = \"local\" for now."
                .to_string(),
        ))
    }

    fn finalize_upload_session(
        &self,
        _vault_path: &str,
        _session_id: &str,
        _target_dir: &str,
        _filename: &str,
    ) -> AppResult<String> {
        Err(AppError::InternalError(
            "S3 storage backend is not implemented yet. Use storage.backend = \"local\" for now."
                .to_string(),
        ))
    }

    fn delete_upload_session_temp(&self, _vault_path: &str, _session_id: &str) -> AppResult<()> {
        Err(AppError::InternalError(
            "S3 storage backend is not implemented yet. Use storage.backend = \"local\" for now."
                .to_string(),
        ))
    }
}

fn upload_temp_file_path(vault_path: &str, session_id: &str) -> std::path::PathBuf {
    Path::new(vault_path)
        .join(".obsidian")
        .join("uploads")
        .join(session_id)
}

fn validate_upload_filename(filename: &str) -> AppResult<()> {
    let file_name_path = Path::new(filename);
    let mut components = file_name_path.components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None) => Ok(()),
        _ => Err(AppError::InvalidInput(
            "Invalid upload filename".to_string(),
        )),
    }
}

pub fn build_storage_backend(config: &StorageConfig) -> Arc<dyn StorageBackend> {
    match config.backend.trim().to_ascii_lowercase().as_str() {
        "s3" => Arc::new(S3StorageBackend),
        _ => Arc::new(LocalFsStorageBackend),
    }
}

pub fn default_storage_backend() -> Arc<dyn StorageBackend> {
    Arc::new(LocalFsStorageBackend)
}
