use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};

pub(super) struct OwnedProcess {
    pub child: Child,
    process_id: Option<u32>,
}

impl OwnedProcess {
    pub fn spawn(mut command: Command) -> Result<Self, String> {
        #[cfg(unix)]
        command.process_group(0);
        let child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| format!("cannot start managed polish process: {error}"))?;
        Ok(Self {
            process_id: child.id(),
            child,
        })
    }

    fn kill_group(&self) {
        #[cfg(unix)]
        if let Some(process_id) = self.process_id.and_then(|value| i32::try_from(value).ok()) {
            unsafe {
                libc::kill(-process_id, libc::SIGKILL);
            }
        }
    }

    pub fn reaped(&mut self) {
        self.kill_group();
        self.process_id = None;
    }

    pub async fn stop(&mut self) -> Result<(), String> {
        self.kill_group();
        if let Err(error) = self.child.start_kill() {
            tracing::debug!(%error, "mlx_polish_process_already_stopped");
        }
        tokio::time::timeout(Duration::from_secs(3), self.child.wait())
            .await
            .map_err(|_| "managed polish process cleanup timed out".to_string())?
            .map_err(|error| format!("cannot reap managed polish process: {error}"))?;
        self.process_id = None;
        Ok(())
    }
}

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        self.kill_group();
    }
}
