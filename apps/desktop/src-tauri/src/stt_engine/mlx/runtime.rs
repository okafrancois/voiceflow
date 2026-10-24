use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::task::JoinSet;

const PYTHON_VERSION: &str = "3.12.15";
const RUNTIME_NAME: &str = "mlx-audio-0.6.1-python-3.12.15";
const REQUIREMENTS: &str = include_str!("requirements.lock");
const READY_SIGNATURE: &str = concat!("python=3.12.15\n", include_str!("requirements.lock"));
const READY_FILE: &str = ".runtime-ready";
const STDERR_LIMIT: usize = 16_384;
const INSTALL_TIMEOUT: Duration = Duration::from_secs(20 * 60);
const VALIDATION: &str = "import sys; from importlib.metadata import version; \
    assert sys.version_info[:3] == (3, 12, 15), sys.version; \
    assert version('mlx-audio') == '0.6.1', version('mlx-audio'); \
    import mlx.core; from mlx_audio.stt.utils import load_model";

static BOOTSTRAP: OnceLock<PathBuf> = OnceLock::new();
static INSTALL_LOCK: Mutex<()> = Mutex::const_new(());

pub fn supported() -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
}

pub fn runtime_dir(models_dir: &Path) -> PathBuf {
    models_dir
        .parent()
        .unwrap_or(models_dir)
        .join("runtimes")
        .join(RUNTIME_NAME)
}

pub fn python_path(models_dir: &Path) -> PathBuf {
    runtime_dir(models_dir).join("bin/python")
}

pub fn installed(models_dir: &Path) -> bool {
    supported() && ready(models_dir)
}

pub(crate) fn invalidate(models_dir: &Path) -> Result<(), String> {
    let marker = runtime_dir(models_dir).join(READY_FILE);
    match std::fs::remove_file(&marker) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "cannot invalidate MLX runtime {}: {error}",
            marker.display()
        )),
    }
}

pub fn configure_bootstrap(path: PathBuf) {
    let _ = BOOTSTRAP.set(path);
}

pub async fn install(models_dir: &Path, cancel: Arc<AtomicBool>) -> Result<(), String> {
    if !supported() {
        return Err("managed MLX transcription requires an Apple Silicon Mac".to_string());
    }
    let result = async {
        check_macos_version(&cancel).await?;
        let bootstrap = BOOTSTRAP.get().cloned().unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/apple-silicon/uv")
        });
        install_with_bootstrap(models_dir, &bootstrap, cancel.clone()).await
    }
    .await;
    normalize_install_result(result, &cancel)
}

fn normalize_install_result(result: Result<(), String>, cancel: &AtomicBool) -> Result<(), String> {
    result.map_err(|error| {
        if cancel.load(Ordering::Relaxed) {
            "cancelled".to_string()
        } else {
            error
        }
    })
}

fn validate_macos_version(version: &str) -> Result<(), String> {
    let components = version
        .trim()
        .split('.')
        .map(|component| {
            if component.is_empty()
                || !component
                    .bytes()
                    .all(|character| character.is_ascii_digit())
            {
                None
            } else {
                component.parse::<u32>().ok()
            }
        })
        .collect::<Option<Vec<_>>>();
    let major = components
        .filter(|components| components.len() <= 3)
        .and_then(|components| components.first().copied())
        .ok_or_else(|| format!(
            "cannot determine macOS version from {version:?}; MLX requires macOS 14 or newer; check Software Update and retry the model download"
        ))?;
    if major < 14 {
        return Err(format!(
            "managed MLX transcription requires macOS 14 or newer (found macOS {}); upgrade macOS in System Settings > General > Software Update, then retry the model download",
            version.trim()
        ));
    }
    Ok(())
}

async fn check_macos_version(cancel: &AtomicBool) -> Result<(), String> {
    let mut command = Command::new("/usr/bin/sw_vers");
    command
        .arg("-productVersion")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .current_dir("/");
    let version = run_command_output(command, "macOS version check", cancel, Duration::from_secs(5)).await
        .map_err(|error| format!("cannot verify the macOS 14 requirement: {error}; check Software Update and retry the model download"))?;
    validate_macos_version(&version)
}

fn ready(models_dir: &Path) -> bool {
    python_path(models_dir).is_file()
        && std::fs::read_to_string(runtime_dir(models_dir).join(READY_FILE))
            .is_ok_and(|contents| contents == READY_SIGNATURE)
}

fn runtime_parent(models_dir: &Path) -> PathBuf {
    runtime_dir(models_dir)
        .parent()
        .expect("versioned runtime always has a parent")
        .to_path_buf()
}

fn python_install_dir(models_dir: &Path) -> PathBuf {
    runtime_parent(models_dir).join("mlx-python")
}

fn cache_dir(models_dir: &Path) -> PathBuf {
    runtime_parent(models_dir).join("mlx-cache")
}

fn check_cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("MLX runtime installation cancelled".to_string())
    } else {
        Ok(())
    }
}

async fn cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn isolated_command(executable: &Path, models_dir: &Path) -> Command {
    let mut command = Command::new(executable);
    command.env_clear();
    for key in [
        "HOME",
        "TMPDIR",
        "TMP",
        "TEMP",
        "SYSTEMROOT",
        "SystemRoot",
        "LANG",
        "LC_ALL",
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "https_proxy",
        "http_proxy",
        "all_proxy",
        "no_proxy",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("UV_PYTHON_INSTALL_DIR", python_install_dir(models_dir))
        .env(
            "UV_PYTHON_BIN_DIR",
            runtime_parent(models_dir).join("mlx-python-bin"),
        )
        .env("UV_CACHE_DIR", cache_dir(models_dir))
        .env("UV_MANAGED_PYTHON", "1")
        .env("UV_NO_CONFIG", "1")
        .env("UV_KEYRING_PROVIDER", "disabled")
        .env("HF_HUB_OFFLINE", "1")
        .env("HF_HUB_DISABLE_TELEMETRY", "1")
        .env("HF_HOME", cache_dir(models_dir).join("huggingface"))
        .current_dir(runtime_parent(models_dir));
    command
}

fn uv_command(bootstrap: &Path, models_dir: &Path) -> Command {
    let mut command = isolated_command(bootstrap, models_dir);
    command.args([
        "--no-config",
        "--managed-python",
        "--no-progress",
        "--color",
        "never",
    ]);
    command
}

async fn validate_runtime(models_dir: &Path, cancel: &AtomicBool) -> Result<(), String> {
    let mut validation = isolated_command(&python_path(models_dir), models_dir);
    validation.args(["-I", "-c", VALIDATION]);
    run_command(
        validation,
        "MLX Python import/version validation",
        cancel,
        Duration::from_secs(120),
    )
    .await
}

async fn install_with_bootstrap(
    models_dir: &Path,
    bootstrap: &Path,
    cancel: Arc<AtomicBool>,
) -> Result<(), String> {
    check_cancelled(&cancel)?;
    if !models_dir.is_absolute() || !bootstrap.is_absolute() {
        return Err(
            "MLX runtime installation requires absolute model and bootstrap paths".to_string(),
        );
    }
    let _guard = tokio::select! {
        guard = INSTALL_LOCK.lock() => guard,
        () = cancelled(&cancel) => return Err("MLX runtime installation cancelled while waiting".to_string()),
    };
    check_cancelled(&cancel)?;
    if ready(models_dir) {
        match validate_runtime(models_dir, &cancel).await {
            Ok(()) => return check_cancelled(&cancel),
            Err(error) if cancel.load(Ordering::Relaxed) => return Err(error),
            Err(_) => invalidate(models_dir)?,
        }
    }
    check_cancelled(&cancel)?;
    if !bootstrap.is_file() {
        return Err(format!(
            "bundled MLX bootstrap missing at {}; run node scripts/ensure-mlx-runtime.mjs or reinstall Voice Flow",
            bootstrap.display()
        ));
    }
    let environment = runtime_dir(models_dir);
    match std::fs::symlink_metadata(&environment) {
        Ok(metadata) => {
            let removal = if metadata.is_dir() {
                std::fs::remove_dir_all(&environment)
            } else {
                std::fs::remove_file(&environment)
            };
            removal.map_err(|error| {
                format!(
                    "cannot reset partial MLX runtime {}: {error}",
                    environment.display()
                )
            })?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "cannot inspect MLX runtime {}: {error}",
                environment.display()
            ))
        }
    }
    for directory in [python_install_dir(models_dir), cache_dir(models_dir)] {
        std::fs::create_dir_all(&directory).map_err(|error| {
            format!(
                "cannot create private MLX directory {}: {error}",
                directory.display()
            )
        })?;
    }

    let mut python = uv_command(bootstrap, models_dir);
    python.args(["python", "install", PYTHON_VERSION, "--no-bin"]);
    run_command(python, "managed Python download", &cancel, INSTALL_TIMEOUT).await?;

    let mut environment_command = uv_command(bootstrap, models_dir);
    environment_command.args(["venv", "--python", PYTHON_VERSION, "--no-python-downloads"]);
    environment_command.arg(&environment);
    run_command(
        environment_command,
        "MLX virtual environment creation",
        &cancel,
        INSTALL_TIMEOUT,
    )
    .await?;

    let lock_path = environment.join("requirements.lock");
    std::fs::write(&lock_path, REQUIREMENTS).map_err(|error| {
        format!(
            "cannot write MLX dependency lock {}: {error}",
            lock_path.display()
        )
    })?;
    let mut dependencies = uv_command(bootstrap, models_dir);
    dependencies.args(["pip", "install", "--python"]);
    dependencies.arg(python_path(models_dir));
    dependencies.args([
        "--no-python-downloads",
        "--require-hashes",
        "--only-binary",
        ":all:",
        "--strict",
        "--default-index",
        "https://pypi.org/simple",
        "--requirements",
    ]);
    dependencies.arg(lock_path);
    run_command(
        dependencies,
        "MLX dependency installation",
        &cancel,
        INSTALL_TIMEOUT,
    )
    .await?;

    validate_runtime(models_dir, &cancel).await?;
    check_cancelled(&cancel)?;
    let temporary_marker = environment.join(".runtime-ready.tmp");
    std::fs::write(&temporary_marker, READY_SIGNATURE)
        .and_then(|()| std::fs::rename(&temporary_marker, environment.join(READY_FILE)))
        .map_err(|error| format!("cannot mark MLX runtime complete: {error}"))?;
    Ok(())
}

async fn collect_output(mut stream: impl AsyncRead + Unpin) -> Result<Vec<u8>, String> {
    let mut tail = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        let count = stream
            .read(&mut buffer)
            .await
            .map_err(|error| format!("cannot read installer output: {error}"))?;
        if count == 0 {
            return Ok(tail);
        }
        if tail.len() + count > STDERR_LIMIT {
            tail.drain(..tail.len() + count - STDERR_LIMIT);
        }
        tail.extend_from_slice(&buffer[..count]);
    }
}

async fn run_command(
    command: Command,
    step: &str,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<(), String> {
    run_command_output(command, step, cancel, timeout)
        .await
        .map(|_| ())
}

async fn run_command_output(
    mut command: Command,
    step: &str,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<String, String> {
    check_cancelled(cancel)?;
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("{step} could not start: {error}"))?;
    let owned_pid = child.id();
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("{step} stderr pipe unavailable"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("{step} stdout pipe unavailable"))?;
    let mut readers = JoinSet::new();
    readers.spawn(async move { collect_output(stderr).await.map(|bytes| (false, bytes)) });
    readers.spawn(async move { collect_output(stdout).await.map(|bytes| (true, bytes)) });
    let outcome = tokio::select! {
        status = child.wait() => status.map_err(|error| format!("{step} failed to wait: {error}")),
        () = cancelled(cancel) => Err(format!("{step} cancelled")),
        () = tokio::time::sleep(timeout) => Err(format!("{step} timed out after {} seconds", timeout.as_secs())),
    };
    if outcome.is_err() {
        stop_installer(&mut child, step, owned_pid).await?;
    }
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    while !readers.is_empty() {
        let output = match tokio::time::timeout(Duration::from_secs(2), readers.join_next()).await {
            Ok(output) => output,
            Err(_) => {
                stop_installer(&mut child, step, owned_pid).await?;
                return Err(format!("{step} installer output pipes did not close"));
            }
        };
        if let Some(result) = output {
            let (is_stdout, bytes) =
                result.map_err(|error| format!("{step} output task failed: {error}"))??;
            if is_stdout {
                stdout = bytes;
            } else {
                stderr = bytes;
            }
        }
    }
    let detail = String::from_utf8_lossy(&stderr);
    match outcome {
        Ok(status) if status.success() => Ok(String::from_utf8_lossy(&stdout).into_owned()),
        Ok(status) => Err(format!("{step} failed ({status}): {}", detail.trim())),
        Err(error) => Err(format!("{error}: {}", detail.trim())),
    }
}

async fn stop_installer(
    child: &mut Child,
    step: &str,
    owned_pid: Option<u32>,
) -> Result<(), String> {
    #[cfg(unix)]
    let group_result = if let Some(pid) = owned_pid {
        let mut terminate = Command::new("/bin/kill");
        terminate
            .args(["-KILL", "--", &format!("-{pid}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        tokio::time::timeout(Duration::from_secs(3), terminate.status())
            .await
            .map_err(|_| format!("{step} process-group cleanup timed out"))
            .and_then(|result| {
                result.map_err(|error| format!("{step} process-group cleanup failed: {error}"))
            })
            .map(|_| ())
    } else {
        Ok(())
    };
    if let Err(error) = child.kill().await {
        child.wait().await.map_err(|wait_error| {
            format!("{step} cleanup for process {owned_pid:?} failed: {error}; {wait_error}")
        })?;
    }
    #[cfg(unix)]
    group_result?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn runtime_paths_are_private_versioned_siblings_of_models() {
        let root = tempfile::tempdir().unwrap();
        let models = root.path().join("user data/models");
        let expected = root
            .path()
            .join("user data/runtimes/mlx-audio-0.6.1-python-3.12.15");
        assert_eq!(runtime_dir(&models), expected);
        assert_eq!(python_path(&models), expected.join("bin/python"));
        assert!(!installed(&models));
        assert_ne!(
            runtime_dir(&models),
            runtime_dir(&root.path().join("other/models"))
        );
    }

    #[test]
    fn support_is_exclusively_native_apple_silicon() {
        assert_eq!(
            supported(),
            cfg!(all(target_os = "macos", target_arch = "aarch64"))
        );
    }

    #[test]
    fn macos_version_validation_rejects_13_with_an_actionable_upgrade_error() {
        for version in ["13", "13.0", "13.7.9"] {
            let error = validate_macos_version(version).unwrap_err();
            assert!(error.contains("macOS 14 or newer"));
            assert!(error.contains(version));
            assert!(error.contains("System Settings > General > Software Update"));
            assert!(error.contains("retry the model download"));
        }
    }

    #[test]
    fn macos_version_validation_accepts_14_and_future_major_versions() {
        for version in [
            "14", "14.0", "14.7.2", "15", "15.2.1", "27", "27.0", " 14.0\n",
        ] {
            assert!(validate_macos_version(version).is_ok(), "{version}");
        }
    }

    #[test]
    fn macos_version_validation_rejects_malformed_probe_output() {
        for version in [
            "",
            " ",
            "macOS 14.0",
            "14.x",
            "14.",
            ".14",
            "14..1",
            "14.0.1.2",
            "14.0beta",
            "14\n15",
            "-14",
            "+14",
            "99999999999999999999",
        ] {
            let error = validate_macos_version(version).unwrap_err();
            assert!(
                error.contains("cannot determine macOS version"),
                "{version}: {error}"
            );
        }
    }

    #[cfg(unix)]
    fn fake_bootstrap(root: &Path) -> PathBuf {
        let executable = root.join("fake uv");
        std::fs::write(
            &executable,
            r#"#!/bin/sh
set -eu
mkdir -p "$UV_CACHE_DIR"
printf '%s\n' "$@" >> "$UV_CACHE_DIR/calls"
printf 'managed=%s\ncache=%s\n' "$UV_PYTHON_INSTALL_DIR" "$UV_CACHE_DIR" >> "$UV_CACHE_DIR/calls"
while [ "$#" -gt 0 ]; do
  case "$1" in python|venv|pip) break;; esac
  shift
done
case "$1" in
  python) exit 0;;
  venv)
    for argument do destination="$argument"; done
    mkdir -p "$destination/bin"
    cat > "$destination/bin/python" <<'PYTHON'
#!/bin/sh
set -eu
if [ -f "$UV_CACHE_DIR/fail-validation" ]; then
  echo 'cannot import mlx.core' >&2
  exit 1
fi
if [ -f "$UV_CACHE_DIR/wait-validation" ]; then
  touch "$UV_CACHE_DIR/validation-started"
  exec /bin/sleep 30
fi
printf 'validated\n' >> "$UV_CACHE_DIR/calls"
PYTHON
    chmod +x "$destination/bin/python";;
  pip)
    if [ -f "$UV_CACHE_DIR/wait-pip" ]; then
      touch "$UV_CACHE_DIR/pip-started"
      exec /bin/sleep 30
    fi
    if [ -f "$UV_CACHE_DIR/fail-pip" ]; then
      echo 'wheel download failed: retry the model download' >&2
      exit 1
    fi;;
  *) exit 2;;
esac
"#,
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        executable
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn install_uses_private_managed_python_and_hashed_lock_then_validates() {
        let root = tempfile::tempdir().unwrap();
        let models = root.path().join("user data/models");
        let bootstrap = fake_bootstrap(root.path());
        install_with_bootstrap(&models, &bootstrap, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        assert!(ready(&models));
        let calls = std::fs::read_to_string(cache_dir(&models).join("calls")).unwrap();
        assert!(calls.contains("--managed-python\n"));
        assert!(calls.contains("install\n3.12.15\n--no-bin\n"));
        assert!(calls.contains("--no-python-downloads\n"));
        assert!(calls.contains("--require-hashes\n"));
        assert!(calls.contains("--only-binary\n:all:\n"));
        assert!(calls.contains("validated\n"));
        assert!(calls.contains(&format!(
            "managed={}\n",
            python_install_dir(&models).display()
        )));
        assert_eq!(
            std::fs::read_to_string(runtime_dir(&models).join("requirements.lock")).unwrap(),
            REQUIREMENTS
        );
        install_with_bootstrap(&models, &bootstrap, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        assert_eq!(
            format!("{calls}validated\n"),
            std::fs::read_to_string(cache_dir(&models).join("calls")).unwrap()
        );
    }

    #[test]
    fn install_boundary_normalizes_cancellation_without_masking_other_results() {
        let cancel = AtomicBool::new(true);
        for error in [
            "MLX runtime installation cancelled",
            "MLX runtime installation cancelled while waiting",
            "cannot verify the macOS 14 requirement: macOS version check cancelled",
            "MLX dependency installation cancelled: interrupted",
            "MLX Python import/version validation cancelled",
        ] {
            assert_eq!(
                normalize_install_result(Err(error.to_string()), &cancel),
                Err("cancelled".to_string())
            );
        }
        assert_eq!(normalize_install_result(Ok(()), &cancel), Ok(()));
        cancel.store(false, Ordering::Relaxed);
        let error = "package download failed after server cancelled request".to_string();
        assert_eq!(
            normalize_install_result(Err(error.clone()), &cancel),
            Err(error)
        );
    }

    #[tokio::test]
    async fn public_install_returns_exact_cancelled_before_platform_probe() {
        if !supported() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let models = root.path().join("models");
        let result = install(&models, Arc::new(AtomicBool::new(true))).await;
        assert_eq!(result, Err("cancelled".to_string()));
        assert!(!runtime_dir(&models).exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn broken_ready_runtime_is_validated_and_rebuilt_before_reuse() {
        let root = tempfile::tempdir().unwrap();
        let models = root.path().join("models");
        let bootstrap = fake_bootstrap(root.path());
        install_with_bootstrap(&models, &bootstrap, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        std::fs::write(
            python_path(&models),
            "#!/bin/sh\necho 'cannot import mlx.core' >&2\nexit 1\n",
        )
        .unwrap();
        let stale_file = runtime_dir(&models).join("broken-package");
        std::fs::write(&stale_file, "broken").unwrap();
        assert!(ready(&models));
        install_with_bootstrap(&models, &bootstrap, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        assert!(ready(&models));
        assert!(!stale_file.exists());
        let calls = std::fs::read_to_string(cache_dir(&models).join("calls")).unwrap();
        assert_eq!(calls.lines().filter(|line| *line == "venv").count(), 2);
        assert_eq!(calls.lines().filter(|line| *line == "validated").count(), 2);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_ready_validation_invalidates_marker_even_when_repair_cannot_start() {
        let root = tempfile::tempdir().unwrap();
        let models = root.path().join("models");
        let bootstrap = fake_bootstrap(root.path());
        install_with_bootstrap(&models, &bootstrap, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        std::fs::write(cache_dir(&models).join("fail-validation"), "fail").unwrap();
        std::fs::remove_file(&bootstrap).unwrap();
        let error = install_with_bootstrap(&models, &bootstrap, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap_err();
        assert!(error.contains("bundled MLX bootstrap missing"), "{error}");
        assert!(!ready(&models));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancelled_reuse_validation_preserves_healthy_environment() {
        let root = tempfile::tempdir().unwrap();
        let models = root.path().join("models");
        let bootstrap = fake_bootstrap(root.path());
        install_with_bootstrap(&models, &bootstrap, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        let sentinel = runtime_dir(&models).join("healthy-package");
        std::fs::write(&sentinel, "healthy").unwrap();
        let calls = std::fs::read_to_string(cache_dir(&models).join("calls")).unwrap();
        std::fs::write(cache_dir(&models).join("wait-validation"), "wait").unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let installation = install_with_bootstrap(&models, &bootstrap, cancel.clone());
        let trigger = async {
            while !cache_dir(&models).join("validation-started").exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            cancel.store(true, Ordering::Relaxed);
        };
        let (result, ()) = tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(installation, trigger)
        })
        .await
        .unwrap();
        assert_eq!(
            normalize_install_result(result, &cancel),
            Err("cancelled".to_string())
        );
        assert!(ready(&models));
        assert_eq!(std::fs::read_to_string(&sentinel).unwrap(), "healthy");
        assert_eq!(
            calls,
            std::fs::read_to_string(cache_dir(&models).join("calls")).unwrap()
        );
        std::fs::remove_file(cache_dir(&models).join("wait-validation")).unwrap();
        install_with_bootstrap(&models, &bootstrap, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        assert!(sentinel.exists());
        assert_eq!(
            format!("{calls}validated\n"),
            std::fs::read_to_string(cache_dir(&models).join("calls")).unwrap()
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_install_and_failed_validation_never_become_ready_and_retry_recovers() {
        let root = tempfile::tempdir().unwrap();
        let models = root.path().join("models");
        let bootstrap = fake_bootstrap(root.path());
        std::fs::create_dir_all(cache_dir(&models)).unwrap();
        for failure in ["fail-pip", "fail-validation"] {
            std::fs::write(cache_dir(&models).join(failure), "fail").unwrap();
            let result =
                install_with_bootstrap(&models, &bootstrap, Arc::new(AtomicBool::new(false))).await;
            assert!(result.is_err());
            assert!(!ready(&models));
            std::fs::remove_file(cache_dir(&models).join(failure)).unwrap();
        }
        std::fs::write(runtime_dir(&models).join("partial"), "stale").unwrap();
        std::fs::write(cache_dir(&models).join("keep-cache"), "cached").unwrap();
        install_with_bootstrap(&models, &bootstrap, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        assert!(ready(&models));
        assert!(!runtime_dir(&models).join("partial").exists());
        assert!(cache_dir(&models).join("keep-cache").exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancelled_dependency_install_is_not_ready_and_retry_recovers() {
        let root = tempfile::tempdir().unwrap();
        let models = root.path().join("models");
        let bootstrap = fake_bootstrap(root.path());
        std::fs::create_dir_all(cache_dir(&models)).unwrap();
        std::fs::write(cache_dir(&models).join("wait-pip"), "wait").unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let installation = install_with_bootstrap(&models, &bootstrap, cancel.clone());
        let trigger = async {
            while !cache_dir(&models).join("pip-started").exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            cancel.store(true, Ordering::Relaxed);
        };
        let (result, ()) = tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(installation, trigger)
        })
        .await
        .unwrap();
        assert_eq!(
            normalize_install_result(result, &cancel),
            Err("cancelled".to_string())
        );
        assert!(!ready(&models));
        std::fs::remove_file(cache_dir(&models).join("wait-pip")).unwrap();
        install_with_bootstrap(&models, &bootstrap, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap();
        assert!(ready(&models));
    }

    #[test]
    fn readiness_requires_the_current_lock_and_python_executable() {
        let root = tempfile::tempdir().unwrap();
        let models = root.path().join("models");
        std::fs::create_dir_all(runtime_dir(&models).join("bin")).unwrap();
        std::fs::write(python_path(&models), "python").unwrap();
        assert!(!ready(&models));
        std::fs::write(runtime_dir(&models).join(READY_FILE), "old lock").unwrap();
        assert!(!ready(&models));
        std::fs::write(runtime_dir(&models).join(READY_FILE), READY_SIGNATURE).unwrap();
        assert!(ready(&models));
        std::fs::remove_file(python_path(&models)).unwrap();
        assert!(!ready(&models));
    }

    #[test]
    fn invalidation_is_idempotent_and_removes_only_the_readiness_marker() {
        let root = tempfile::tempdir().unwrap();
        let models = root.path().join("models");
        std::fs::create_dir_all(runtime_dir(&models).join("bin")).unwrap();
        std::fs::create_dir_all(cache_dir(&models)).unwrap();
        std::fs::create_dir_all(&models).unwrap();
        std::fs::write(python_path(&models), "python").unwrap();
        std::fs::write(runtime_dir(&models).join(READY_FILE), READY_SIGNATURE).unwrap();
        let preserved = [
            runtime_dir(&models).join("package"),
            cache_dir(&models).join("cached-wheel"),
            models.join("checkpoint"),
        ];
        for file in &preserved {
            std::fs::write(file, "preserve").unwrap();
        }
        assert!(ready(&models));
        invalidate(&models).unwrap();
        assert!(!ready(&models));
        assert_eq!(
            std::fs::read_to_string(python_path(&models)).unwrap(),
            "python"
        );
        for file in &preserved {
            assert_eq!(std::fs::read_to_string(file).unwrap(), "preserve");
        }
        invalidate(&models).unwrap();
        let missing_models = root.path().join("missing-user/models");
        invalidate(&missing_models).unwrap();
        assert!(!runtime_dir(&missing_models).exists());
    }

    #[test]
    fn invalidation_reports_marker_io_failures_without_removing_other_files() {
        let root = tempfile::tempdir().unwrap();
        let models = root.path().join("models");
        let marker = runtime_dir(&models).join(READY_FILE);
        std::fs::create_dir_all(&marker).unwrap();
        let error = invalidate(&models).unwrap_err();
        assert!(error.contains("cannot invalidate MLX runtime"), "{error}");
        assert!(error.contains(&marker.display().to_string()), "{error}");
        assert!(marker.is_dir());
    }

    #[test]
    fn installer_commands_isolate_environment_and_working_directory() {
        let root = tempfile::tempdir().unwrap();
        let models = root.path().join("models");
        let command = isolated_command(Path::new("/fake/uv"), &models);
        assert_eq!(
            command.as_std().get_current_dir(),
            Some(runtime_parent(&models).as_path())
        );
        let environment: std::collections::HashMap<_, _> = command.as_std().get_envs().collect();
        assert_eq!(
            environment[std::ffi::OsStr::new("UV_MANAGED_PYTHON")],
            Some(std::ffi::OsStr::new("1"))
        );
        assert_eq!(
            environment[std::ffi::OsStr::new("PATH")],
            Some(std::ffi::OsStr::new("/usr/bin:/bin:/usr/sbin:/sbin"))
        );
        for key in [
            "PYTHONPATH",
            "PYTHONHOME",
            "VIRTUAL_ENV",
            "CONDA_PREFIX",
            "UV_INDEX_URL",
            "UV_PYTHON",
        ] {
            assert!(!environment.contains_key(std::ffi::OsStr::new(key)));
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_while_waiting_for_install_lock_returns_without_touching_environment() {
        let root = tempfile::tempdir().unwrap();
        let models = root.path().join("models");
        let bootstrap = fake_bootstrap(root.path());
        let guard = INSTALL_LOCK.lock().await;
        let cancel = Arc::new(AtomicBool::new(false));
        let pending = install_with_bootstrap(&models, &bootstrap, cancel.clone());
        let cancelled = async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            cancel.store(true, Ordering::Relaxed);
        };
        let (result, ()) = tokio::time::timeout(Duration::from_secs(2), async {
            tokio::join!(pending, cancelled)
        })
        .await
        .unwrap();
        assert_eq!(
            normalize_install_result(result, &cancel),
            Err("cancelled".to_string())
        );
        assert!(!runtime_dir(&models).exists());
        drop(guard);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn subprocess_errors_include_bounded_stderr_tail() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "count=0; while [ \"$count\" -lt 4000 ]; do echo padding-padding-padding >&2; count=$((count+1)); done; echo actionable-last-error >&2; exit 7"]);
        let result = run_command(
            command,
            "test installer",
            &AtomicBool::new(false),
            Duration::from_secs(5),
        )
        .await
        .unwrap_err();
        assert!(result.contains("actionable-last-error"));
        assert!(result.contains("7"));
        assert!(result.len() < STDERR_LIMIT + 200);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timeout_and_cancel_kill_and_reap_only_the_owned_process() {
        let root = tempfile::tempdir().unwrap();
        let mut unrelated = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap();
        for cancelled in [false, true] {
            let pid_file = root.path().join(format!("pid-{cancelled}"));
            let mut command = Command::new("/bin/sh");
            command.args([
                "-c",
                "echo $$ > \"$1\"; exec /bin/sleep 30",
                "installer-test",
            ]);
            command.arg(&pid_file);
            let cancel = AtomicBool::new(false);
            let result = run_command(
                command,
                "test installer",
                &cancel,
                Duration::from_millis(200),
            );
            let trigger = async {
                if cancelled {
                    while !pid_file.exists() {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                    cancel.store(true, Ordering::Relaxed);
                }
            };
            let (error, ()) = tokio::join!(result, trigger);
            assert!(error
                .unwrap_err()
                .contains(if cancelled { "cancelled" } else { "timed out" }));
            let pid = std::fs::read_to_string(&pid_file).unwrap();
            let alive = std::process::Command::new("/bin/kill")
                .args(["-0", pid.trim()])
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success();
            assert!(!alive, "owned installer must be reaped");
            assert!(unrelated.try_wait().unwrap().is_none());
        }
        unrelated.kill().unwrap();
        unrelated.wait().unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timeout_also_stops_installer_owned_descendants() {
        let root = tempfile::tempdir().unwrap();
        let pid_file = root.path().join("descendant pid");
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "/bin/sleep 30 & echo $! > \"$1\"; wait",
            "installer-test",
        ]);
        command.arg(&pid_file);
        let error = run_command(
            command,
            "test installer",
            &AtomicBool::new(false),
            Duration::from_millis(200),
        )
        .await
        .unwrap_err();
        let pid = std::fs::read_to_string(&pid_file).unwrap();
        let alive = std::process::Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        if alive {
            std::process::Command::new("/bin/kill")
                .args(["-KILL", pid.trim()])
                .status()
                .unwrap();
        }
        assert!(error.contains("timed out"), "{error}");
        assert!(!alive, "installer descendant must not outlive a timeout");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn output_timeout_stops_descendants_even_when_installer_has_already_exited() {
        let root = tempfile::tempdir().unwrap();
        let pid_file = root.path().join("descendant pid");
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "/bin/sleep 30 & echo $! > \"$1\"; exit 0",
            "installer-test",
        ]);
        command.arg(&pid_file);
        let error = run_command(
            command,
            "test installer",
            &AtomicBool::new(false),
            Duration::from_secs(5),
        )
        .await
        .unwrap_err();
        let pid = std::fs::read_to_string(&pid_file).unwrap();
        let alive = std::process::Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        if alive {
            std::process::Command::new("/bin/kill")
                .args(["-KILL", pid.trim()])
                .status()
                .unwrap();
        }
        assert!(error.contains("output pipes did not close"), "{error}");
        assert!(!alive, "orphaned installer descendant must be stopped");
    }

    #[tokio::test]
    async fn unsupported_platforms_do_not_create_runtime_files() {
        if supported() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let models = root.path().join("models");
        assert!(install(&models, Arc::new(AtomicBool::new(false)))
            .await
            .unwrap_err()
            .contains("Apple Silicon"));
        assert!(!runtime_dir(&models).exists());
    }
}
