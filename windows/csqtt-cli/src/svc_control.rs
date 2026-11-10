//! Service lifetime tied to the app: the window (or the tray) starts the service when it
//! opens, and the last of the two to close stops it. `csqtt-svc setup` grants interactive
//! users the right to start and stop the service, so none of this needs a UAC prompt.

use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

use crate::gui::NamedClaim;

const SERVICE_NAME: &str = "csqtt-win";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Held by the running window (see gui.rs) and by the running tray.
pub(crate) const GUI_MUTEX: &str = "Local\\csqtt-win-gui";
pub(crate) const TRAY_MUTEX: &str = "Local\\csqtt-win-tray";

fn sc(args: &[&str]) -> Option<i32> {
    Command::new("sc.exe")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .ok()
        .and_then(|s| s.code())
}

/// Starts the service if it is installed and stopped. Errors are ignored: when the service
/// is missing the window shows the setup button, and a running one answers with 1056.
pub(crate) fn start_service() {
    let _ = sc(&["start", SERVICE_NAME]);
}

/// Stops the service (graceful: it tears the tunnel, routes and DNS down first).
fn stop_service() {
    let _ = sc(&["stop", SERVICE_NAME]);
}

/// Called by the window or the tray right before it exits: stops the service unless the
/// other one is still running (its mutex is held).
pub(crate) fn stop_if_last(other_mutex: &str) {
    if NamedClaim::claim(other_mutex).is_some() {
        stop_service();
    }
}

/// True while the service is starting or running (sc query STATE 2 = START_PENDING,
/// 4 = RUNNING): the pipe may not be listening yet, so callers should wait for it.
pub(crate) fn starting_or_running() -> bool {
    let Ok(out) = Command::new("sc.exe")
        .args(["query", SERVICE_NAME])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return false;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find(|l| l.contains("STATE"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.split_whitespace().next())
        .is_some_and(|code| code == "2" || code == "4")
}
