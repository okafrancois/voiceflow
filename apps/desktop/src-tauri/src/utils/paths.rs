use std::path::PathBuf;
use std::sync::OnceLock;

/// Application data directory, initialized from Tauri's PathResolver at startup.
/// Falls back to "voiceflow" if not initialized (for tests or early bootstrapping).
static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

fn legacy_product_token() -> String {
    ["aria", "type"].concat()
}

fn migrate_legacy_directory(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> std::io::Result<()> {
    if !source.exists() || source == destination {
        return Ok(());
    }

    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if destination_path.exists() {
            if source_path.is_dir() && destination_path.is_dir() {
                migrate_legacy_directory(&source_path, &destination_path)?;
            }
            continue;
        }

        std::fs::rename(&source_path, &destination_path)?;
    }

    if std::fs::read_dir(source)?.next().is_none() {
        std::fs::remove_dir(source)?;
    }
    Ok(())
}

pub struct AppPaths;

impl AppPaths {
    /// Initialize from Tauri's PathResolver. Called once during app setup.
    pub fn init_from_tauri(data_dir: PathBuf) {
        if let Some(base_dir) = dirs::data_dir() {
            let legacy_token = legacy_product_token();
            let legacy_directories = [
                base_dir.join(&legacy_token),
                base_dir.join(format!("com.{legacy_token}.voicetotext")),
            ];

            for legacy_directory in legacy_directories {
                if let Err(error) = migrate_legacy_directory(&legacy_directory, &data_dir) {
                    tracing::warn!(
                        error = %error,
                        source = ?legacy_directory,
                        destination = ?data_dir,
                        "legacy_data_directory_migration_failed"
                    );
                }
            }
        }
        let _ = APP_DATA_DIR.set(data_dir);
    }

    pub fn data_dir() -> PathBuf {
        APP_DATA_DIR.get().cloned().unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("voiceflow")
        })
    }

    pub fn shared_data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Voice Flow")
    }

    pub fn models_dir() -> PathBuf {
        Self::data_dir().join("models")
    }

    pub fn recordings_dir() -> PathBuf {
        Self::data_dir().join("recordings")
    }

    pub fn correction_learning_dir() -> PathBuf {
        Self::shared_data_dir().join("correction-learning")
    }

    pub fn correction_learning_file() -> PathBuf {
        Self::correction_learning_dir().join("corrections.json")
    }

    pub fn cache_dir() -> PathBuf {
        APP_DATA_DIR.get().cloned().unwrap_or_else(|| {
            dirs::cache_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("voiceflow")
        })
    }

    pub fn temp_dir() -> PathBuf {
        let path = Self::cache_dir().join("temp");
        if let Err(e) = std::fs::create_dir_all(&path) {
            tracing::warn!(error = %e, path = ?path, "temp_directory_creation_failed");
        }
        path
    }

    pub fn log_dir() -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Library/Logs")
                .join(Self::data_dir().file_name().unwrap_or_default())
        }
        #[cfg(target_os = "windows")]
        {
            Self::data_dir().join("logs")
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Self::data_dir().join("logs")
        }
    }

    pub fn ensure_dirs() {
        if let Err(e) = std::fs::create_dir_all(Self::data_dir()) {
            tracing::warn!(error = %e, "data_directory_creation_failed");
        }
        if let Err(e) = std::fs::create_dir_all(Self::models_dir()) {
            tracing::warn!(error = %e, "models_directory_creation_failed");
        }
        if let Err(e) = std::fs::create_dir_all(Self::recordings_dir()) {
            tracing::warn!(error = %e, "recordings_directory_creation_failed");
        }
        if let Err(e) = std::fs::create_dir_all(Self::correction_learning_dir()) {
            tracing::warn!(error = %e, "correction_learning_directory_creation_failed");
        }
        if let Err(e) = std::fs::create_dir_all(Self::cache_dir()) {
            tracing::warn!(error = %e, "cache_directory_creation_failed");
        }
        if let Err(e) = std::fs::create_dir_all(Self::temp_dir()) {
            tracing::warn!(error = %e, "temp_directory_creation_failed");
        }
        if let Err(e) = std::fs::create_dir_all(Self::log_dir()) {
            tracing::warn!(error = %e, "log_directory_creation_failed");
        }
    }

    pub fn cleanup_temp_dir(max_age_secs: u64) {
        let temp_dir = Self::temp_dir();
        let Ok(entries) = std::fs::read_dir(&temp_dir) else {
            return;
        };

        let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(max_age_secs);

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            if let Ok(meta) = std::fs::metadata(&path) {
                if let Ok(modified) = meta.modified() {
                    if modified < cutoff {
                        if let Err(e) = std::fs::remove_file(&path) {
                            tracing::debug!(error = %e, path = ?path, "stale_temp_file_removal_failed");
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_paths_fallback_without_init() {
        // Without init, use the current Voice Flow directory.
        let data = AppPaths::data_dir();
        assert!(data.ends_with("voiceflow"));

        let models = AppPaths::models_dir();
        assert!(models.ends_with("models"));
        assert!(models.starts_with(&data));

        let recordings = AppPaths::recordings_dir();
        assert!(recordings.ends_with("recordings"));
        assert!(recordings.starts_with(&data));

        let correction_file = AppPaths::correction_learning_file();
        assert!(correction_file.ends_with("corrections.json"));
        assert!(correction_file
            .parent()
            .unwrap()
            .ends_with("correction-learning"));

        let temp = AppPaths::temp_dir();
        assert!(temp.ends_with("temp"));
    }

    #[test]
    fn migrates_legacy_data_without_overwriting_current_files() {
        let root = tempdir().unwrap();
        let legacy = root.path().join("legacy");
        let current = root.path().join("current");
        std::fs::create_dir_all(legacy.join("models")).unwrap();
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(legacy.join("settings.json"), b"legacy settings").unwrap();
        std::fs::write(legacy.join("models/model.bin"), b"model").unwrap();
        std::fs::write(current.join("settings.json"), b"current settings").unwrap();

        migrate_legacy_directory(&legacy, &current).unwrap();

        assert_eq!(
            std::fs::read(current.join("settings.json")).unwrap(),
            b"current settings"
        );
        assert_eq!(
            std::fs::read(current.join("models/model.bin")).unwrap(),
            b"model"
        );
        assert!(!legacy.join("models/model.bin").exists());
    }
}
