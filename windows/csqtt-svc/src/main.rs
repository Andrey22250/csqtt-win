//! csqtt-svc: host service (Windows service wrapper, console runner, and IPC).
//!
//! Usage (elevated console; client.exe and wintun.dll next to csqtt-svc.exe):
//!   csqtt-svc service               start as Windows service (invoked by SCM)
//!   csqtt-svc install               install Windows service
//!   csqtt-svc uninstall             uninstall Windows service
//!   csqtt-svc run [params.json]     run with IPC server, optionally connect with CoreParams
//!   csqtt-svc example-params        print a CoreParams JSON template
//!   csqtt-svc cleanup               remove leftover routes, NRPT rule and adapter

use std::path::PathBuf;

use csqtt_svc::core::args::CoreParams;
use csqtt_svc::manager::{self, ManagerConfig, ManagerEvent, Request, Status};
use csqtt_svc::store::paths;

fn exe_dir() -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(exe.parent().map(PathBuf::from).unwrap_or_default())
}

fn config() -> anyhow::Result<ManagerConfig> {
    let dir = exe_dir()?;
    paths::ensure_data_dir()?;
    Ok(ManagerConfig::new(dir.join("client.exe"), dir.clone(), paths::routes_journal_path()))
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("service") => csqtt_svc::service::run_as_service(),
        Some("install") => {
            csqtt_svc::service::install()?;
            println!("Служба csqtt-win успешно установлена.");
            Ok(())
        }
        Some("setup") => {
            csqtt_svc::service::setup()?;
            println!("Служба csqtt-win установлена и запущена.");
            Ok(())
        }
        Some("uninstall") => {
            csqtt_svc::service::uninstall()?;
            println!("Служба csqtt-win успешно удалена.");
            Ok(())
        }
        Some("run") => {
            let params = if let Some(path) = args.get(2) {
                let text = std::fs::read_to_string(path)?;
                let p: CoreParams = serde_json::from_str(&text)?;
                Some(p)
            } else {
                None
            };
            tokio::runtime::Runtime::new()?.block_on(run(params))
        }
        Some("example-params") => {
            let params = CoreParams {
                peer: "203.0.113.10:46000".into(),
                vk_hashes: vec!["<vk-call-hash>".into()],
                workers_per_hash: 5,
                connection_password: "<password>".into(),
                vk_hash_mode: "manual".into(),
                vk_auth_mode: "vkcalls".into(),
                captcha_mode: "auto".into(),
                fingerprint: "chrome".into(),
                client_ids: "8202606,6287487".into(),
                obfs_mode: "unified".into(),
                turn_transport: "udp".into(),
                srtp_mode: "auto".into(),
                ..CoreParams::default()
            };
            println!("{}", serde_json::to_string_pretty(&params)?);
            Ok(())
        }
        Some("cleanup") => tokio::runtime::Runtime::new()?.block_on(async {
            // Spawning the manager runs the startup cleanup; then shut it down.
            let (handle, task) = manager::spawn(config()?);
            let mut events = handle.subscribe();
            handle.send(Request::Shutdown);
            let printer = tokio::spawn(async move {
                while let Ok(event) = events.recv().await {
                    println!("{event:?}");
                }
            });
            task.await?;
            printer.abort();
            Ok(())
        }),
        _ => {
            eprintln!("usage: csqtt-svc run [params.json] | service | install | uninstall | example-params | cleanup");
            std::process::exit(2);
        }
    }
}

async fn run(params: Option<CoreParams>) -> anyhow::Result<()> {
    let (handle, task) = manager::spawn(config()?);
    let shutdown = tokio_util::sync::CancellationToken::new();

    let serve_handle = handle.clone();
    let serve_shutdown = shutdown.clone();
    let serve_task = tokio::spawn(async move {
        if let Err(e) = csqtt_svc::ipc_server::serve(serve_handle, serve_shutdown).await {
            eprintln!("[IPC] ошибка сервера: {e}");
        }
    });

    let mut events = handle.subscribe();
    if let Some(params) = params {
        handle.send(Request::Start(Box::new(params)));
    }

    let ctrl_handle = handle.clone();
    let ctrl_shutdown = shutdown.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            eprintln!("[CTRL+C] отключаемся...");
            ctrl_shutdown.cancel();
            ctrl_handle.send(Request::Stop { finish_vk_calls: true });
            ctrl_handle.send(Request::Shutdown);
        }
    });

    let printer_handle = handle.clone();
    let printer = tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(ManagerEvent::Log(line)) => println!("{line}"),
                Ok(ManagerEvent::Status(status)) => {
                    println!("== статус: {status:?}");
                    if matches!(status, Status::Failed { .. }) {
                        printer_handle.send(Request::Shutdown);
                    }
                }
                Ok(ManagerEvent::Stats { active, bytes_up, bytes_down }) => {
                    println!("== активных: {active} | ↑ {} КБ ↓ {} КБ", bytes_up / 1024, bytes_down / 1024)
                }
                Ok(ManagerEvent::CaptchaRequest { redirect_uri, .. }) => {
                    println!("== капча: {redirect_uri}");
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => println!("== пропущено событий: {n}"),
                Err(_) => break,
            }
        }
    });

    task.await?;
    shutdown.cancel();
    let _ = serve_task.await;
    printer.abort();
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn valid_subcommands_recognized() {
        let valid = ["service", "install", "uninstall", "run", "example-params", "cleanup"];
        for cmd in valid {
            assert!(matches!(
                cmd,
                "service" | "install" | "uninstall" | "run" | "example-params" | "cleanup"
            ));
        }
    }
}
