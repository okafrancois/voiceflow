mod engine;
mod integrity;
pub mod models;
mod process;
pub mod runtime;

pub use engine::MlxPolishEngine;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::utils::{download, remove_download_artifacts, DownloadOptions};

async fn download_file(
    options: DownloadOptions,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<(), String> {
    let mut artifacts = PartialDownload {
        preserve_existing: options.output_path.exists(),
        path: options.output_path.clone(),
        complete: false,
    };
    tokio::select! {
        biased;
        () = runtime::cancelled(&cancel) => return Err("cancelled".to_string()),
        result = tokio::time::timeout(timeout, download(options)) => {
            result.map_err(|_| "managed polish checkpoint download timed out; retry the download".to_string())??;
        }
    }
    runtime::check_cancelled(&cancel)?;
    artifacts.complete = true;
    Ok(())
}

struct PartialDownload {
    path: PathBuf,
    preserve_existing: bool,
    complete: bool,
}

impl Drop for PartialDownload {
    fn drop(&mut self) {
        if self.complete {
            return;
        }
        if self.preserve_existing {
            let temporary = self.path.with_file_name(format!(
                "{}.tmp",
                self.path.file_name().unwrap_or_default().to_string_lossy()
            ));
            if let Err(error) = std::fs::remove_file(temporary) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(%error, "mlx_polish_partial_download_cleanup_failed");
                }
            }
        } else {
            remove_download_artifacts(&self.path);
        }
    }
}

struct StopHashing(Arc<AtomicBool>);

impl Drop for StopHashing {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

async fn verify_file(
    path: PathBuf,
    file: models::ModelFile,
    cancel: Arc<AtomicBool>,
) -> Result<(), String> {
    runtime::check_cancelled(&cancel)?;
    let started = Instant::now();
    let stopped = Arc::new(AtomicBool::new(false));
    let _stop = StopHashing(stopped.clone());
    let external_cancel = cancel.clone();
    let hashing = tokio::task::spawn_blocking(move || {
        runtime::check_cancelled(&cancel)?;
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|error| format!("cannot inspect polish model file: {error}"))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != file.bytes
        {
            return Err(format!(
                "incomplete or unsafe polish model file: {}",
                file.name
            ));
        }
        let mut input = std::fs::File::open(&path)
            .map_err(|error| format!("cannot open polish model file: {error}"))?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65_536];
        loop {
            runtime::check_cancelled(&cancel)?;
            runtime::check_cancelled(&stopped)?;
            if started.elapsed() >= integrity::VERIFY_TIMEOUT {
                return Err("managed polish checkpoint verification timed out".to_string());
            }
            let count = input
                .read(&mut buffer)
                .map_err(|error| format!("cannot verify polish model file: {error}"))?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        if format!("{:x}", hash.finalize()) != file.sha256 {
            return Err(format!("polish model checksum mismatch: {}", file.name));
        }
        Ok(())
    });
    tokio::select! {
        biased;
        () = runtime::cancelled(&external_cancel) => Err("cancelled".to_string()),
        result = tokio::time::timeout(integrity::VERIFY_TIMEOUT, hashing) => {
            result.map_err(|_| "managed polish checkpoint verification timed out".to_string())?
                .map_err(|error| format!("polish model verification task failed: {error}"))?
        }
    }
}

pub async fn download_model(
    models_dir: &Path,
    id: &str,
    cancel: Arc<AtomicBool>,
    progress: Arc<dyn Fn(u64, u64) + Send + Sync>,
) -> Result<(), String> {
    let model = models::find(id).ok_or_else(|| format!("unknown managed polish model: {id}"))?;
    runtime::check_cancelled(&cancel)?;
    runtime::install(models_dir, cancel.clone()).await?;
    std::fs::create_dir_all(models_dir)
        .map_err(|error| format!("cannot create polish models directory: {error}"))?;
    let directory = models_dir.join(model.id);
    match std::fs::symlink_metadata(&directory) {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            return Err("unsafe managed polish model directory".to_string())
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&directory).map_err(|error| {
                format!("cannot create managed polish model directory: {error}")
            })?;
        }
        Err(error) => {
            return Err(format!(
                "cannot inspect managed polish model directory: {error}"
            ))
        }
    }
    let marker = directory.join(".model-ready");
    if marker.exists() {
        std::fs::remove_file(&marker)
            .map_err(|error| format!("cannot reset polish model readiness: {error}"))?;
    }
    let total = model.files.iter().map(|file| file.bytes).sum();
    let mut completed = 0;
    for file in model.files {
        runtime::check_cancelled(&cancel)?;
        let path = directory.join(file.name);
        if std::fs::symlink_metadata(&path)
            .is_ok_and(|metadata| metadata.file_type().is_symlink() || !metadata.is_file())
        {
            return Err(format!("unsafe managed polish model file: {}", file.name));
        }
        let callback = progress.clone();
        let offset = completed;
        let length = file.bytes;
        let url = format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            model.repository, model.revision, file.name
        );
        download_file(
            DownloadOptions::new(url, &path)
                .with_cancel_flag(cancel.clone())
                .with_minimum_bytes(file.bytes)
                .with_model_name(id)
                .with_progress_callback(Arc::new(move |downloaded, _| {
                    callback(offset + downloaded.min(length), total)
                })),
            cancel.clone(),
            Duration::from_secs(20 * 60),
        )
        .await?;
        if let Err(error) = verify_file(path.clone(), *file, cancel.clone()).await {
            if error != "cancelled" {
                std::fs::remove_file(&path).map_err(|removal| {
                    format!("{error}; cannot remove invalid polish file: {removal}")
                })?;
                remove_download_artifacts(&path);
            }
            return Err(error);
        }
        completed += file.bytes;
        progress(completed, total);
    }
    runtime::check_cancelled(&cancel)?;
    let temporary = directory.join(".model-ready.tmp");
    std::fs::write(&temporary, models::signature(model))
        .and_then(|()| std::fs::rename(temporary, marker))
        .map_err(|error| format!("cannot publish managed polish model readiness: {error}"))
}

#[cfg(test)]
#[path = "download_tests.rs"]
mod download_tests;

#[cfg(test)]
#[path = "integrity_tests.rs"]
mod integrity_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn verifies_exact_checkpoint_file_hashes_before_readiness() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.json");
        let file = models::ModelFile {
            name: "config.json",
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        };
        std::fs::write(&path, b"abc").unwrap();
        verify_file(path.clone(), file, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        std::fs::write(&path, b"abd").unwrap();
        assert!(
            verify_file(path.clone(), file, Arc::new(AtomicBool::new(false)))
                .await
                .unwrap_err()
                .contains("checksum mismatch")
        );
        std::fs::write(&path, b"ab").unwrap();
        assert!(
            verify_file(path.clone(), file, Arc::new(AtomicBool::new(false)))
                .await
                .is_err()
        );
        assert_eq!(
            verify_file(path, file, Arc::new(AtomicBool::new(true)))
                .await
                .unwrap_err(),
            "cancelled"
        );
        assert!(!root.path().join(".model-ready").exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_symlinked_checkpoint_files() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("outside.json");
        let path = root.path().join("config.json");
        std::fs::write(&target, b"abc").unwrap();
        std::os::unix::fs::symlink(target, &path).unwrap();
        let file = models::ModelFile {
            name: "config.json",
            bytes: 3,
            sha256: "ignored",
        };
        assert!(verify_file(path, file, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap_err()
            .contains("unsafe"));
    }
}
