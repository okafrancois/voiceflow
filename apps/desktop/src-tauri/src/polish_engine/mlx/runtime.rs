use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::task::JoinSet;

use super::process::OwnedProcess;

const REQUIREMENTS: &str = include_str!("requirements.lock");
const SIGNATURE: &str = concat!("python=3.12.15\n", include_str!("requirements.lock"));
const VALIDATION: &str = "import sys; from importlib.metadata import version; \
    assert sys.version_info[:3] == (3, 12, 15), sys.version; \
    assert version('mlx-lm') == '0.32.2', version('mlx-lm'); \
    assert version('mlx') == '0.32.4', version('mlx'); \
    from mlx_lm import load, stream_generate; from mlx_lm.sample_utils import make_sampler";
const INSTALL_TIMEOUT: Duration = Duration::from_secs(20 * 60);
static BOOTSTRAP: OnceLock<PathBuf> = OnceLock::new();
static INSTALL_LOCK: Mutex<()> = Mutex::const_new(());

#[cfg(test)]
mod eligibility_tests {
    use super::*;

    #[test]
    fn old_unknown_and_malformed_macos_versions_are_not_eligible() {
        for version in [
            None,
            Some("12.7.6"),
            Some("13"),
            Some("13.6.9"),
            Some(""),
            Some("macOS 14"),
            Some("14.bad"),
            Some("14.0.0.1"),
            Some("14."),
            Some("14\n15"),
        ] {
            assert_eq!(
                eligibility("macos", "aarch64", version),
                Err("os_unsupported"),
                "{version:?}"
            );
        }
    }

    #[test]
    fn macos_14_and_newer_apple_silicon_are_eligible() {
        for version in ["14", "14.0", "14.6.1", "15.5", "26.0.1"] {
            assert!(
                eligibility("macos", "aarch64", Some(version)).is_ok(),
                "{version}"
            );
        }
    }

    #[test]
    fn unsupported_platforms_stay_ineligible_regardless_of_version() {
        for (platform, arch) in [
            ("macos", "x86_64"),
            ("windows", "aarch64"),
            ("linux", "aarch64"),
            ("unknown", "aarch64"),
        ] {
            assert_eq!(
                eligibility(platform, arch, Some("26.0")),
                Err("platform_unsupported")
            );
        }
    }
}

pub fn supported() -> bool {
    ensure_supported().is_ok()
}

pub fn eligibility(platform: &str, arch: &str, version: Option<&str>) -> Result<(), &'static str> {
    if platform != "macos" || arch != "aarch64" {
        return Err("platform_unsupported");
    }
    let components = version
        .ok_or("os_unsupported")?
        .trim()
        .split('.')
        .map(|component| {
            if !component.is_empty() && component.bytes().all(|byte| byte.is_ascii_digit()) {
                component.parse::<u32>().ok()
            } else {
                None
            }
        })
        .collect::<Option<Vec<_>>>()
        .ok_or("os_unsupported")?;
    if components.len() > 3 || components.first().is_none_or(|major| *major < 14) {
        return Err("os_unsupported");
    }
    Ok(())
}

pub fn ensure_supported() -> Result<(), String> {
    eligibility(
        std::env::consts::OS,
        std::env::consts::ARCH,
        macos_version(),
    )
    .map_err(|code| match code {
        "platform_unsupported" => "managed MLX polish requires an Apple Silicon Mac".to_string(),
        _ => "managed MLX polish requires verified macOS 14 or newer; upgrade macOS and retry"
            .to_string(),
    })
}

pub fn macos_version() -> Option<&'static str> {
    static VERSION: OnceLock<Option<String>> = OnceLock::new();
    VERSION.get_or_init(detect_macos_version).as_deref()
}

#[cfg(target_os = "macos")]
fn detect_macos_version() -> Option<String> {
    use cocoa::base::nil;
    use cocoa::foundation::NSProcessInfo;
    let version = unsafe { NSProcessInfo::processInfo(nil).operatingSystemVersion() };
    Some(format!(
        "{}.{}.{}",
        version.majorVersion, version.minorVersion, version.patchVersion
    ))
}

#[cfg(not(target_os = "macos"))]
fn detect_macos_version() -> Option<String> {
    None
}

pub fn configure_bootstrap(path: PathBuf) {
    let _ = BOOTSTRAP.set(path);
}

pub fn runtime_dir(models_dir: &Path) -> PathBuf {
    models_dir
        .parent()
        .unwrap_or(models_dir)
        .join("runtimes")
        .join("mlx-lm-0.32.2-python-3.12.15")
}

pub fn python_path(models_dir: &Path) -> PathBuf {
    runtime_dir(models_dir).join("bin/python")
}

pub fn installed(models_dir: &Path) -> bool {
    supported()
        && std::fs::symlink_metadata(runtime_dir(models_dir))
            .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        && python_path(models_dir).is_file()
        && std::fs::read_to_string(runtime_dir(models_dir).join(".runtime-ready"))
            .is_ok_and(|value| value == SIGNATURE)
}

pub(super) fn invalidate(models_dir: &Path) -> Result<(), String> {
    let path = runtime_dir(models_dir).join(".runtime-ready");
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot invalidate managed polish runtime: {error}")),
    }
}

pub(super) async fn cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
}

pub(super) fn check_cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("cancelled".to_string())
    } else {
        Ok(())
    }
}

pub(super) fn isolated_command(executable: &Path, models_dir: &Path) -> Command {
    let parent = runtime_dir(models_dir)
        .parent()
        .expect("runtime parent")
        .to_path_buf();
    let mut command = Command::new(executable);
    command.env_clear();
    for key in [
        "HOME",
        "TMPDIR",
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
        .env("UV_PYTHON_INSTALL_DIR", parent.join("mlx-polish-python"))
        .env("UV_PYTHON_BIN_DIR", parent.join("mlx-polish-python-bin"))
        .env("UV_CACHE_DIR", parent.join("mlx-polish-cache"))
        .env("UV_MANAGED_PYTHON", "1")
        .env("UV_NO_CONFIG", "1")
        .env("UV_KEYRING_PROVIDER", "disabled")
        .env("HF_HUB_OFFLINE", "1")
        .env("TRANSFORMERS_OFFLINE", "1")
        .env("HF_HUB_DISABLE_TELEMETRY", "1")
        .env("HF_HOME", parent.join("mlx-polish-cache/huggingface"))
        .current_dir(parent);
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

async fn validate(models_dir: &Path, cancel: &AtomicBool) -> Result<(), String> {
    let mut command = isolated_command(&python_path(models_dir), models_dir);
    command.args(["-I", "-c", VALIDATION]);
    run(command, cancel, Duration::from_secs(120))
        .await
        .map(|_| ())
}

pub async fn install(models_dir: &Path, cancel: Arc<AtomicBool>) -> Result<(), String> {
    let result = install_inner(models_dir, &cancel).await;
    if cancel.load(Ordering::Relaxed) {
        Err("cancelled".to_string())
    } else {
        result
    }
}

async fn install_inner(models_dir: &Path, cancel: &AtomicBool) -> Result<(), String> {
    check_cancelled(cancel)?;
    ensure_supported()?;
    if !models_dir.is_absolute() {
        return Err("managed polish requires an absolute model directory".to_string());
    }
    let _guard = tokio::select! {
        guard = INSTALL_LOCK.lock() => guard,
        () = cancelled(cancel) => return Err("cancelled".to_string()),
    };
    let parent = runtime_dir(models_dir)
        .parent()
        .expect("runtime parent")
        .to_path_buf();
    std::fs::create_dir_all(&parent)
        .map_err(|error| format!("cannot create private polish runtime directory: {error}"))?;
    if installed(models_dir) {
        match validate(models_dir, cancel).await {
            Ok(()) => return check_cancelled(cancel),
            Err(error) if cancel.load(Ordering::Relaxed) => return Err(error),
            Err(_) => invalidate(models_dir)?,
        }
    }
    let bootstrap = BOOTSTRAP
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/apple-silicon/uv"));
    if !bootstrap.is_file() {
        return Err("bundled polish bootstrap unavailable; reinstall Voice Flow".to_string());
    }
    let environment = runtime_dir(models_dir);
    match std::fs::symlink_metadata(&environment) {
        Ok(metadata) => {
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                std::fs::remove_dir_all(&environment)
            } else {
                std::fs::remove_file(&environment)
            }
            .map_err(|error| format!("cannot reset incomplete polish runtime: {error}"))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect polish runtime: {error}")),
    }
    let mut python = uv_command(&bootstrap, models_dir);
    python.args(["python", "install", "3.12.15", "--no-bin"]);
    run(python, cancel, INSTALL_TIMEOUT).await?;
    let mut venv = uv_command(&bootstrap, models_dir);
    venv.args(["venv", "--python", "3.12.15", "--no-python-downloads"])
        .arg(&environment);
    run(venv, cancel, INSTALL_TIMEOUT).await?;
    let lock = environment.join("requirements.lock");
    std::fs::write(&lock, REQUIREMENTS)
        .map_err(|error| format!("cannot write polish runtime lock: {error}"))?;
    let mut dependencies = uv_command(&bootstrap, models_dir);
    dependencies
        .args(["pip", "install", "--python"])
        .arg(python_path(models_dir))
        .args([
            "--no-python-downloads",
            "--require-hashes",
            "--only-binary",
            ":all:",
            "--strict",
            "--default-index",
            "https://pypi.org/simple",
            "--requirements",
        ])
        .arg(lock);
    run(dependencies, cancel, INSTALL_TIMEOUT).await?;
    validate(models_dir, cancel).await?;
    check_cancelled(cancel)?;
    let temporary = environment.join(".runtime-ready.tmp");
    std::fs::write(&temporary, SIGNATURE)
        .and_then(|()| std::fs::rename(temporary, environment.join(".runtime-ready")))
        .map_err(|error| format!("cannot publish polish runtime readiness: {error}"))
}

async fn collect(mut stream: impl AsyncRead + Unpin) -> Result<Vec<u8>, String> {
    let mut tail = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let count = stream
            .read(&mut buffer)
            .await
            .map_err(|error| format!("cannot read polish installer output: {error}"))?;
        if count == 0 {
            return Ok(tail);
        }
        if tail.len() + count > 16_384 {
            tail.drain(..tail.len() + count - 16_384);
        }
        tail.extend_from_slice(&buffer[..count]);
    }
}

async fn run(command: Command, cancel: &AtomicBool, timeout: Duration) -> Result<String, String> {
    check_cancelled(cancel)?;
    let mut process = OwnedProcess::spawn(command)?;
    process.child.stdin.take();
    let stdout = process
        .child
        .stdout
        .take()
        .ok_or("polish installer stdout missing")?;
    let stderr = process
        .child
        .stderr
        .take()
        .ok_or("polish installer stderr missing")?;
    let mut readers = JoinSet::new();
    readers.spawn(async move { collect(stdout).await.map(|bytes| (true, bytes)) });
    readers.spawn(async move { collect(stderr).await.map(|bytes| (false, bytes)) });
    let outcome = tokio::select! {
        status = process.child.wait() => status.map_err(|error| format!("cannot wait for polish installer: {error}")),
        () = cancelled(cancel) => Err("cancelled".to_string()),
        () = tokio::time::sleep(timeout) => Err("managed polish installation timed out".to_string()),
    };
    if outcome.is_err() {
        process.stop().await?;
    } else {
        process.reaped();
    }
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    while !readers.is_empty() {
        let output = tokio::time::timeout(Duration::from_secs(3), readers.join_next())
            .await
            .map_err(|_| "polish installer output cleanup timed out".to_string())?;
        if let Some(result) = output {
            let (is_stdout, bytes) = result
                .map_err(|error| format!("polish installer output task failed: {error}"))??;
            if is_stdout {
                stdout = bytes;
            } else {
                stderr = bytes;
            }
        }
    }
    match outcome {
        Ok(status) if status.success() => Ok(String::from_utf8_lossy(&stdout).into_owned()),
        Ok(status) => Err(format!(
            "managed polish installer failed ({status}): {}",
            String::from_utf8_lossy(&stderr).trim()
        )),
        Err(error) => Err(error),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    async fn wait_for_pid(path: &Path) -> u32 {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Ok(pid) = std::fs::read_to_string(path) {
                    if let Ok(pid) = pid.trim().parse() {
                        return pid;
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }

    async fn assert_stopped(process_id: u32) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if !Command::new("/bin/kill")
                    .args(["-0", &process_id.to_string()])
                    .stderr(std::process::Stdio::null())
                    .status()
                    .await
                    .unwrap()
                    .success()
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn installer_cancellation_stops_its_owned_process_group() {
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("pid");
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("sleep 60 & echo $! > \"$1\"; wait")
            .arg("owned-installer")
            .arg(&marker);
        let cancel = Arc::new(AtomicBool::new(false));
        let owned_cancel = cancel.clone();
        let installer =
            tokio::spawn(async move { run(command, &owned_cancel, Duration::from_secs(60)).await });
        let process_id = wait_for_pid(&marker).await;
        cancel.store(true, Ordering::Relaxed);
        assert_eq!(installer.await.unwrap().unwrap_err(), "cancelled");
        assert_stopped(process_id).await;
    }

    #[tokio::test]
    async fn dropping_installation_future_cannot_orphan_its_children() {
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("pid");
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("sleep 60 & echo $! > \"$1\"; wait")
            .arg("owned-installer")
            .arg(&marker);
        let installer = tokio::spawn(async move {
            run(command, &AtomicBool::new(false), Duration::from_secs(60)).await
        });
        let process_id = wait_for_pid(&marker).await;
        installer.abort();
        assert!(installer.await.unwrap_err().is_cancelled());
        assert_stopped(process_id).await;
    }

    #[tokio::test]
    async fn installer_deadline_and_large_diagnostic_output_are_bounded() {
        let mut command = Command::new("/bin/sleep");
        command.arg("60");
        assert!(
            run(command, &AtomicBool::new(false), Duration::from_millis(30))
                .await
                .unwrap_err()
                .contains("timed out")
        );
        let mut output = Command::new("/usr/bin/python3");
        output.args([
            "-I",
            "-c",
            "import sys; sys.stdout.write('x'*200000); sys.stderr.write('y'*200000)",
        ]);
        let result = run(output, &AtomicBool::new(false), Duration::from_secs(3))
            .await
            .unwrap();
        assert_eq!(result.len(), 16_384);
    }
}
