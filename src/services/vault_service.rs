use crate::error::{AppError, AppResult};
use std::fs::{self, File};
use std::io::{self};
use std::path::{Path, PathBuf};
use tracing::{debug, info};
use zip::ZipArchive;

pub struct VaultService;

impl VaultService {
    /// Extract a ZIP file to the specified vault directory.
    /// Cleans the directory first if `overwrite` is true (default for alpha: true).
    pub fn extract_zip(zip_file: File, target_dir: &Path) -> AppResult<()> {
        let mut archive = ZipArchive::new(zip_file).map_err(|e| {
            AppError::InternalError(format!("Failed to open ZIP archive: {}", e))
        })?;

        info!("Extracting ZIP to {:?}", target_dir);

        // Ensure target directory exists
        if !target_dir.exists() {
            fs::create_dir_all(target_dir).map_err(|e| {
                AppError::InternalError(format!("Failed to create vault directory: {}", e))
            })?;
        }

        // Extract files
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| {
                AppError::InternalError(format!("Failed to read ZIP entry {}: {}", i, e))
            })?;

            let outpath = match file.enclosed_name() {
                Some(path) => target_dir.join(path),
                None => continue,
            };

            if (*file.name()).ends_with('/') {
                fs::create_dir_all(&outpath).map_err(|e| {
                    AppError::InternalError(format!("Failed to create directory {:?}: {}", outpath, e))
                })?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        fs::create_dir_all(p).map_err(|e| {
                            AppError::InternalError(format!("Failed to create parent directory {:?}: {}", p, e))
                        })?;
                    }
                }
                let mut outfile = fs::File::create(&outpath).map_err(|e| {
                    AppError::InternalError(format!("Failed to create file {:?}: {}", outpath, e))
                })?;
                io::copy(&mut file, &mut outfile).map_err(|e| {
                    AppError::InternalError(format!("Failed to write file {:?}: {}", outpath, e))
                })?;
            }

            // Get and set permissions (if unix)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))
                        .map_err(|e| AppError::InternalError(format!("Failed to set permissions: {}", e)))?;
                }
            }
        }

        debug!("Extraction complete for {:?}", target_dir);
        Ok(())
    }
}
