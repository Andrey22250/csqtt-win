//! Windows service (SCM) wrapper, install/uninstall.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use tokio_util::sync::CancellationToken;
use windows_service::{
    define_windows_service,
    service::{
        ServiceAccess, ServiceAction, ServiceActionType, ServiceControl, ServiceControlAccept,
        ServiceErrorControl, ServiceExitCode, ServiceFailureActions, ServiceFailureResetPeriod,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

pub const SERVICE_NAME: &str = "csqtt-win";
pub const SERVICE_DISPLAY_NAME: &str = "csqtt-win VPN";
pub const SERVICE_DESCRIPTION: &str = "csqtt VPN client host service";

define_windows_service!(ffi_service_main, my_service_main);

/// Entry point when launched by Windows Service Control Manager.
pub fn run_as_service() -> anyhow::Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
        .context("failed to start service dispatcher")
}

fn my_service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service_worker() {
        eprintln!("service worker error: {e:?}");
    }
}

fn run_service_worker() -> anyhow::Result<()> {
    let shutdown_token = CancellationToken::new();
    let token_clone = shutdown_token.clone();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop | ServiceControl::Shutdown => {
                token_clone.cancel();
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)
        .context("failed to register service control handler")?;

    // Report StartPending
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 1,
        wait_hint: Duration::from_secs(10),
        process_id: None,
    })?;

    let rt = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;

    let result = rt.block_on(async {
        let exe = std::env::current_exe()?;
        let exe_dir = exe.parent().map(PathBuf::from).unwrap_or_default();
        // Service runs as SYSTEM: lock the data dir down to SYSTEM + Administrators.
        if let Err(e) = crate::store::paths::secure_data_dir() {
            eprintln!("secure_data_dir: {e:#}");
            crate::store::paths::ensure_data_dir()?;
        }
        let file_log = crate::filelog::FileLog::open(&crate::store::paths::logs_dir(), "service", 5 * 1024 * 1024, 5).ok();
        let config = crate::manager::ManagerConfig::new(
            exe_dir.join("client.exe"),
            exe_dir.clone(),
            crate::store::paths::routes_journal_path(),
        );

        let (manager, manager_task) = crate::manager::spawn(config);
        let logger_task = file_log.as_ref().map(|log| {
            log.write_line(&format!("== служба запущена, версия {}", env!("CARGO_PKG_VERSION")));
            log.spawn_manager_logger(manager.subscribe())
        });

        // Transition to Running
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        let serve_manager = manager.clone();
        let serve_shutdown = shutdown_token.clone();
        let serve_log = file_log.clone();
        let serve_task = tokio::spawn(async move {
            if let Err(e) = crate::ipc_server::serve(serve_manager, serve_shutdown).await {
                eprintln!("ipc_server error: {e}");
                if let Some(log) = serve_log {
                    log.write_line(&format!("[СЛУЖБА] IPC-сервер остановлен с ошибкой: {e:#}"));
                }
            }
        });

        // Wait until Stop or Shutdown is requested
        shutdown_token.cancelled().await;

        // Report StopPending
        let _ = status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::StopPending,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 1,
            wait_hint: Duration::from_secs(15),
            process_id: None,
        });

        // Stop manager and await task
        let _ = serve_task.await;
        manager.send(crate::manager::Request::Shutdown);
        let _ = manager_task.await;
        if let Some(task) = logger_task {
            task.abort();
        }
        if let Some(log) = &file_log {
            log.write_line("== служба остановлена");
        }

        Ok::<(), anyhow::Error>(())
    });

    let exit_code = match result {
        Ok(()) => ServiceExitCode::Win32(0),
        Err(ref e) => {
            eprintln!("service runtime error: {e:?}");
            ServiceExitCode::Win32(1)
        }
    };

    // Report Stopped
    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code,
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    });

    result
}

/// Builds ServiceInfo for installing csqtt-win.
pub fn build_service_info(executable_path: PathBuf) -> ServiceInfo {
    ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path,
        launch_arguments: vec![OsString::from("service")],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    }
}

/// Builds failure actions restarting the service on failure (5s, 10s, 20s; reset after 24h).
pub fn build_failure_actions() -> ServiceFailureActions {
    let actions = vec![
        ServiceAction {
            action_type: ServiceActionType::Restart,
            delay: Duration::from_secs(5),
        },
        ServiceAction {
            action_type: ServiceActionType::Restart,
            delay: Duration::from_secs(10),
        },
        ServiceAction {
            action_type: ServiceActionType::Restart,
            delay: Duration::from_secs(20),
        },
    ];

    ServiceFailureActions {
        reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(86400)),
        reboot_msg: None,
        command: None,
        actions: Some(actions),
    }
}

/// Installs the Windows service with autostart and restart failure actions.
pub fn install() -> anyhow::Result<()> {
    let service_manager = ServiceManager::local_computer(
        None::<&OsStr>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    ).context("failed to connect to service control manager")?;

    let exe_path = std::env::current_exe().context("failed to determine executable path")?;
    let service_info = build_service_info(exe_path);

    let service = service_manager
        .create_service(&service_info, ServiceAccess::CHANGE_CONFIG)
        .context("failed to create service")?;

    service
        .set_description(SERVICE_DESCRIPTION)
        .context("failed to set service description")?;

    let failure_actions = build_failure_actions();
    let _ = service.update_failure_actions(failure_actions);

    Ok(())
}

/// One-step setup for the GUI: installs the service if it is missing (an existing one is
/// pointed at this exe, e.g. after the folder was moved), then starts it and waits until it
/// runs. Idempotent; needs administrator rights.
pub fn setup() -> anyhow::Result<()> {
    let service_manager = ServiceManager::local_computer(
        None::<&OsStr>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    ).context("failed to connect to service control manager")?;
    let access = ServiceAccess::QUERY_STATUS | ServiceAccess::START | ServiceAccess::CHANGE_CONFIG;
    let service = match service_manager.open_service(SERVICE_NAME, access) {
        Ok(service) => {
            let exe_path = std::env::current_exe().context("failed to determine executable path")?;
            service
                .change_config(&build_service_info(exe_path))
                .context("failed to update service configuration")?;
            service
        }
        Err(windows_service::Error::Winapi(e))
            if e.raw_os_error()
                == Some(windows_sys::Win32::Foundation::ERROR_SERVICE_DOES_NOT_EXIST as i32) =>
        {
            install()?;
            service_manager
                .open_service(SERVICE_NAME, access)
                .context("failed to open the installed service")?
        }
        Err(e) => return Err(e).context("failed to open service"),
    };

    if service.query_status()?.current_state == ServiceState::Stopped {
        service.start(&[] as &[&OsStr]).context("failed to start service")?;
    }
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        if service.query_status()?.current_state == ServiceState::Running {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    anyhow::bail!("служба не запустилась за 20 секунд")
}

/// Stops the service if running and uninstalls it.
pub fn uninstall() -> anyhow::Result<()> {
    let service_manager = ServiceManager::local_computer(
        None::<&OsStr>,
        ServiceManagerAccess::CONNECT,
    ).context("failed to connect to service control manager")?;

    let service = match service_manager.open_service(
        SERVICE_NAME,
        ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
    ) {
        Ok(s) => s,
        Err(windows_service::Error::Winapi(e))
            if e.raw_os_error()
                == Some(windows_sys::Win32::Foundation::ERROR_SERVICE_DOES_NOT_EXIST as i32) =>
        {
            return Ok(());
        }
        Err(e) => return Err(e).context("failed to open service"),
    };

    if let Ok(status) = service.query_status() {
        if status.current_state != ServiceState::Stopped {
            let _ = service.stop();
            let start = std::time::Instant::now();
            let timeout = Duration::from_secs(15);
            while start.elapsed() < timeout {
                if let Ok(st) = service.query_status() {
                    if st.current_state == ServiceState::Stopped {
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }

    if let Err(windows_service::Error::Winapi(e)) = service.delete() {
        if e.raw_os_error()
            != Some(windows_sys::Win32::Foundation::ERROR_SERVICE_MARKED_FOR_DELETE as i32)
        {
            return Err(windows_service::Error::Winapi(e)).context("failed to delete service");
        }
    }

    drop(service);

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(10);
    while start.elapsed() < timeout {
        match service_manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS) {
            Err(windows_service::Error::Winapi(e))
                if e.raw_os_error()
                    == Some(windows_sys::Win32::Foundation::ERROR_SERVICE_DOES_NOT_EXIST as i32) =>
            {
                return Ok(());
            }
            _ => std::thread::sleep(Duration::from_millis(250)),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_info_configuration() {
        let path = PathBuf::from(r"C:\csqtt\csqtt-svc.exe");
        let info = build_service_info(path.clone());
        assert_eq!(info.name, OsString::from(SERVICE_NAME));
        assert_eq!(info.display_name, OsString::from(SERVICE_DISPLAY_NAME));
        assert_eq!(info.service_type, ServiceType::OWN_PROCESS);
        assert_eq!(info.start_type, ServiceStartType::AutoStart);
        assert_eq!(info.error_control, ServiceErrorControl::Normal);
        assert_eq!(info.executable_path, path);
        assert_eq!(info.launch_arguments, vec![OsString::from("service")]);
        assert!(info.dependencies.is_empty());
        assert!(info.account_name.is_none());
    }

    #[test]
    fn failure_actions_configuration() {
        let fa = build_failure_actions();
        assert_eq!(
            fa.reset_period,
            ServiceFailureResetPeriod::After(Duration::from_secs(86400))
        );
        let actions = fa.actions.expect("actions must be set");
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].action_type, ServiceActionType::Restart);
        assert_eq!(actions[0].delay, Duration::from_secs(5));
        assert_eq!(actions[1].action_type, ServiceActionType::Restart);
        assert_eq!(actions[1].delay, Duration::from_secs(10));
        assert_eq!(actions[2].action_type, ServiceActionType::Restart);
        assert_eq!(actions[2].delay, Duration::from_secs(20));
    }
}
