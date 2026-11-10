//! The core child process.
use std::path::Path;

use super::args::CoreCommand;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreExit {
    /// exit code 0
    Normal,
    /// exit code 1
    Fatal,
    /// exit code 3
    TunLost,
    Other(i32),
    /// killed by us or no exit code
    Killed,
}

impl CoreExit {
    pub fn from_code(code: Option<i32>) -> Self {
        match code {
            Some(0) => CoreExit::Normal,
            Some(1) => CoreExit::Fatal,
            Some(3) => CoreExit::TunLost,
            Some(n) => CoreExit::Other(n),
            None => CoreExit::Killed,
        }
    }
}

/// RAII wrapper for Windows Job object handle.
struct JobHandle(windows_sys::Win32::Foundation::HANDLE);

// SAFETY: Windows kernel HANDLEs are process-wide identifiers safe to transfer across threads.
unsafe impl Send for JobHandle {}
unsafe impl Sync for JobHandle {}

impl Drop for JobHandle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            // SAFETY: Closing valid Job object handle.
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.0);
            }
        }
    }
}

/// Spawns a background task reading lines from a pipe, decoding lossy UTF-8 and stripping trailing '\r'.
fn spawn_line_reader<R>(reader: R, tx: tokio::sync::mpsc::UnboundedSender<String>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;
        let mut buf_reader = tokio::io::BufReader::new(reader);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match buf_reader.read_until(b'\n', &mut buf).await {
                Ok(0) => break, // EOF reached
                Ok(_) => {
                    while buf.ends_with(b"\n") || buf.ends_with(b"\r") {
                        buf.pop();
                    }
                    let line = String::from_utf8_lossy(&buf).into_owned();
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

/// Running core: stdout+stderr lines go to `lines`, stdin accepts CoreCommand.
/// The child is put in a Job object with KILL_ON_JOB_CLOSE and CREATE_NO_WINDOW.
pub struct CoreProcess {
    pub pid: u32,
    child: tokio::process::Child,
    stdin: Option<tokio::process::ChildStdin>,
    _job: JobHandle,
}

impl CoreProcess {
    pub async fn spawn(
        exe: &Path,
        args: &[String],
        env: &[(String, String)],
        lines: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> anyhow::Result<Self> {
        let mut cmd = tokio::process::Command::new(exe);
        cmd.args(args);
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
            .kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("failed to spawn core process at '{}': {e}", exe.display()))?;

        // SAFETY: Creating an unnamed job object with default security attributes.
        let job = unsafe {
            windows_sys::Win32::System::JobObjects::CreateJobObjectW(
                core::ptr::null(),
                core::ptr::null(),
            )
        };
        if job.is_null() {
            // SAFETY: GetLastError is always safe to call immediately after a Win32 API failure.
            let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            anyhow::bail!("CreateJobObjectW failed with error code {err}");
        }
        let job_handle = JobHandle(job);

        let mut info = windows_sys::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags =
            windows_sys::Win32::System::JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        // SAFETY: Setting extended limit info on a valid job object handle.
        let ok = unsafe {
            windows_sys::Win32::System::JobObjects::SetInformationJobObject(
                job_handle.0,
                windows_sys::Win32::System::JobObjects::JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                core::mem::size_of::<windows_sys::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok == 0 {
            // SAFETY: GetLastError is safe after failure.
            let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            anyhow::bail!("SetInformationJobObject failed with error code {err}");
        }

        let raw_handle = child
            .raw_handle()
            .ok_or_else(|| anyhow::anyhow!("child process handle is not available"))?;

        // SAFETY: Assigning child process to job object using valid handles.
        let ok = unsafe {
            windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(
                job_handle.0,
                raw_handle as windows_sys::Win32::Foundation::HANDLE,
            )
        };
        if ok == 0 {
            // SAFETY: GetLastError is safe after failure.
            let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            anyhow::bail!("AssignProcessToJobObject failed with error code {err}");
        }

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("child stdout was not piped"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("child stderr was not piped"))?;

        spawn_line_reader(stdout, lines.clone());
        spawn_line_reader(stderr, lines);

        let pid = child
            .id()
            .ok_or_else(|| anyhow::anyhow!("failed to retrieve child process pid"))?;

        let stdin = child.stdin.take();

        Ok(Self {
            pid,
            child,
            stdin,
            _job: job_handle,
        })
    }

    pub async fn send(&mut self, command: CoreCommand) -> anyhow::Result<()> {
        use tokio::io::AsyncWriteExt;
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("child stdin is closed or unavailable"))?;
        let line = command.line();
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("failed to write command to core stdin: {e}"))?;
        stdin
            .flush()
            .await
            .map_err(|e| anyhow::anyhow!("failed to flush core stdin: {e}"))?;
        Ok(())
    }

    /// Wait for exit.
    pub async fn wait(&mut self) -> CoreExit {
        // Keep stdin open: callers select! on wait() while still sending commands
        // (child.wait() is cancel-safe).
        let code = self.child.wait().await.ok().and_then(|s| s.code());
        CoreExit::from_code(code)
    }

    /// TunnelManager.terminateExactProcess: write FINISH_VK_CALLS (optional) + STOP, wait 3200 ms,
    /// then kill and wait 500 ms.
    pub async fn terminate(&mut self, finish_vk_calls: bool) -> CoreExit {
        use tokio::io::AsyncWriteExt;
        if let Some(mut stdin) = self.stdin.take() {
            if finish_vk_calls {
                let _ = stdin
                    .write_all(CoreCommand::FinishVkCalls.line().as_bytes())
                    .await;
            }
            let _ = stdin.write_all(CoreCommand::Stop.line().as_bytes()).await;
            let _ = stdin.flush().await;
            drop(stdin);
        }

        // Wait gracefully up to 3200 ms
        match tokio::time::timeout(std::time::Duration::from_millis(3200), self.child.wait()).await {
            Ok(Ok(status)) => return CoreExit::from_code(status.code()),
            Ok(Err(_)) => return CoreExit::Killed,
            Err(_) => {} // Timeout elapsed, proceed to kill
        }

        // Force kill
        let _ = self.child.start_kill();

        // Wait up to 500 ms for process termination
        match tokio::time::timeout(std::time::Duration::from_millis(500), self.child.wait()).await {
            Ok(Ok(status)) => CoreExit::from_code(status.code()),
            _ => CoreExit::Killed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_exit_from_code() {
        assert_eq!(CoreExit::from_code(Some(0)), CoreExit::Normal);
        assert_eq!(CoreExit::from_code(Some(1)), CoreExit::Fatal);
        assert_eq!(CoreExit::from_code(Some(3)), CoreExit::TunLost);
        assert_eq!(CoreExit::from_code(Some(2)), CoreExit::Other(2));
        assert_eq!(CoreExit::from_code(Some(-1)), CoreExit::Other(-1));
        assert_eq!(CoreExit::from_code(None), CoreExit::Killed);
    }
}
