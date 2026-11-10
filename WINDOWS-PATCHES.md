# Отличия от апстрима

Каждое изменение файлов апстрима (`rust-client/` и др.) фиксируем здесь: файл, что и зачем. При merge апстрима сверяемся с этим списком.

| Файл | Изменение | Зачем |
|------|-----------|-------|
| `rust-client/Cargo.toml`, `Cargo.lock` | секция `[target.'cfg(windows)'.dependencies]`: `wintun 0.5.1`, `windows-sys 0.61.2` | Wintun-бэкенд TUN, WinAPI для watchdog и сокетов. На Unix/Android не влияет |
| `rust-client/tun.rs` | `#![cfg_attr(windows, allow(unused_imports))]`; заглушка `FdReceiver` → `cfg(not(any(unix, windows)))`; `#[cfg(windows)] mod windows` (`tun_windows.rs`) + `pub use windows::{FdReceiver, TunSession}` | Wintun вместо TUN fd: ядро слушает named pipe `\.\pipe\<tun-uds>`, хост пишет имя адаптера + `\n`, ядро открывает адаптер, стартует сессию, отвечает ACK `[1]` (`[0]` при ошибке) |
| `rust-client/dispatcher.rs` | `cfg_attr(windows, allow(unused_imports))` на `use std::{…}`; заглушка `run_tun` → `cfg(not(any(unix, windows)))`; в конце файла `#[cfg(windows)] mod windows_tun` (`dispatcher_windows.rs`) | `run_tun`/`read_tun`/`write_tun` для Wintun с той же семантикой (горячая замена, TUN_LOST, backoff) |
| `rust-client/main.rs` | `#[cfg(windows)] mod platform_windows;`; в `start_parent_monitor` ветка `cfg(windows)` → `platform_windows::watch_parent` | Watchdog родителя (ToolHelp + `WaitForSingleObject`) |
| `rust-client/turn.rs`, `dns.rs`, `direct_relay.rs` (×2) | `#[cfg(windows)] prepare_udp(&sock, ipv6)` после `bind` | `IP_UNICAST_IF`/`IPV6_UNICAST_IF` из env `CSQTT_BYPASS_IFINDEX` (защита от петли) + `SIO_UDP_CONNRESET`=FALSE (иначе ICMP unreachable рвёт UDP-цикл) |
| `rust-client/turn_stream.rs`, `direct_relay.rs` | TCP connect через `platform_windows::connect_tcp` под `cfg(windows)`; `cfg_attr(windows, allow(unused_imports))` на `use tokio::{…}` | То же для TCP |
| `rust-client/auth.rs`, `vk_js_calls.rs` | `#[cfg(windows)]` `.local_address(platform_windows::bypass_local_ip())` у primp-клиентов | HTTP VK API мимо туннеля: env `CSQTT_BYPASS_LOCAL_IP`; Windows strong host model держит трафик на интерфейсе адреса. Патч vendor/primp не нужен |
| `rust-client/main.rs` | `#[cfg(windows)] #[path = "app_exclude_windows.rs"] pub mod app_exclude_windows;` | Per-app исключения (этап 7) |
| `rust-client/direct_relay.rs` | приватные `app_rules_active()` / `app_excluded()` (на не-Windows всегда `false`); ранний выход `try_handle` пропускается при активных правилах; `is_candidate_direct ||= app_excluded(protocol, src)` (TCP — на SYN, UDP — каждый пакет) | Трафик процессов из env `CSQTT_EXCLUDED_APPS` (exe/пути через `\|`) уходит существующим direct-путём мимо туннеля; DNS не исключается |
| новые: `tun_windows.rs`, `dispatcher_windows.rs`, `platform_windows.rs`, `app_exclude_windows.rs` | весь Windows-код ядра (`app_exclude_windows`: порт → PID через `GetExtendedTcpTable`/`GetExtendedUdpTable`, PID → exe через `QueryFullProcessImageNameW`, кэши 30/60 с) | В апстриме их нет — конфликтов при merge не будет |
