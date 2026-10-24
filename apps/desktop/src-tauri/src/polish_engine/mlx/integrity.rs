use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use tokio::sync::Mutex;

use super::{models, runtime, verify_file, StopHashing};

pub(super) const VERIFY_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, PartialEq, Eq)]
struct FileIdentity {
    bytes: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    changed: (i64, i64),
}

impl FileIdentity {
    fn read(path: &Path) -> Result<Self, String> {
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|error| format!("cannot inspect managed polish checkpoint: {error}"))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("unsafe managed polish checkpoint file".to_string());
        }
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Ok(Self {
            bytes: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            #[cfg(unix)]
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Fingerprint {
    manifest: String,
    files: Vec<FileIdentity>,
}

fn fingerprint(models_dir: &Path, model: &models::Model) -> Result<Fingerprint, String> {
    if !models::files_complete_for(models_dir, model) {
        return Err(
            "managed polish checkpoint is incomplete; download the selected model".to_string(),
        );
    }
    Ok(Fingerprint {
        manifest: models::signature(model),
        files: model
            .files
            .iter()
            .map(|file| FileIdentity::read(&models_dir.join(model.id).join(file.name)))
            .collect::<Result<_, _>>()?,
    })
}

#[derive(Default)]
pub(super) struct ModelIntegrity {
    verified: Mutex<HashMap<PathBuf, Fingerprint>>,
}

impl ModelIntegrity {
    pub async fn verify(
        &self,
        models_dir: &Path,
        model: &models::Model,
        cancel: Arc<AtomicBool>,
        timeout: Duration,
    ) -> Result<(), String> {
        let started = Instant::now();
        check(&cancel, started, timeout)?;
        let hashing_cancel = Arc::new(AtomicBool::new(false));
        let _stop = StopHashing(hashing_cancel.clone());
        tokio::select! {
            biased;
            () = runtime::cancelled(&cancel) => Err("cancelled".to_string()),
            result = tokio::time::timeout(timeout, self.verify_inner(models_dir, model, &cancel, hashing_cancel, started, timeout)) => {
                result.map_err(|_| "managed polish checkpoint verification timed out".to_string())?
            }
        }
    }

    async fn verify_inner(
        &self,
        models_dir: &Path,
        model: &models::Model,
        cancel: &AtomicBool,
        hashing_cancel: Arc<AtomicBool>,
        started: Instant,
        timeout: Duration,
    ) -> Result<(), String> {
        let mut cache = self.verified.lock().await;
        check(cancel, started, timeout)?;
        let directory = models_dir.join(model.id);
        let before = fingerprint(models_dir, model)?;
        if cache.get(&directory) == Some(&before) {
            return Ok(());
        }
        cache.remove(&directory);
        for (file, identity) in model.files.iter().zip(&before.files) {
            check(cancel, started, timeout)?;
            let path = directory.join(file.name);
            let result = verify_file(path.clone(), *file, hashing_cancel.clone()).await;
            check(cancel, started, timeout)?;
            if let Err(error) = result {
                if !error.starts_with("polish model checksum mismatch:") {
                    return Err(error);
                }
                if FileIdentity::read(&path).as_ref() != Ok(identity) {
                    invalidate(&directory)?;
                    return Err(
                        "managed polish checkpoint changed during verification; retry download"
                            .to_string(),
                    );
                }
                invalidate(&directory)?;
                std::fs::remove_file(&path).map_err(|removal| {
                    format!("{error}; cannot remove corrupt checkpoint: {removal}")
                })?;
                crate::utils::remove_download_artifacts(&path);
                return Err(error);
            }
        }
        check(cancel, started, timeout)?;
        if fingerprint(models_dir, model)? != before {
            invalidate(&directory)?;
            return Err(
                "managed polish checkpoint changed during verification; retry download".to_string(),
            );
        }
        cache.insert(directory, before);
        Ok(())
    }
}

fn check(cancel: &AtomicBool, started: Instant, timeout: Duration) -> Result<(), String> {
    runtime::check_cancelled(cancel)?;
    if started.elapsed() >= timeout {
        Err("managed polish checkpoint verification timed out".to_string())
    } else {
        Ok(())
    }
}

fn invalidate(directory: &Path) -> Result<(), String> {
    match std::fs::remove_file(directory.join(".model-ready")) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "cannot invalidate corrupt polish checkpoint readiness: {error}"
        )),
    }
}
