# csqtt-win

Windows-порт csqtt 2.5.0 (апстрим: github.com/amurcanov/csqtt). Полный бриф, критерии готовности и план этапов — в `BRIEF.md`. Работаем по нему.

## Правила форка
- Репозиторий — форк апстрима с историей, remote `upstream`. Обновление: `git fetch upstream && git merge upstream/main`.
- `app/`, `rust-server/`, `shared/` апстрима не меняем. `app/` — эталон поведения (Kotlin) и дизайна (Compose).
- В `rust-client/` — только минимальные изолированные патчи под `cfg(windows)`, по возможности в новых файлах (`*_windows.rs`). Каждый патч сразу записываем в `WINDOWS-PATCHES.md`.
- Весь Windows-код — в `windows/` (Cargo workspace): `csqtt-svc` (служба), `csqtt` (CLI + трей), общие крейты.

## Окружение
- Rust 1.97.1 MSVC: `RUSTUP_HOME`/`CARGO_HOME` = `D:\Claude main\toolchains\rust\{rustup,cargo}` (в PATH пользователя).
- MSVC: Visual Studio Community 2026.
- Тестовый стенд: свой сервер csqtt + VK-аккаунт у пользователя.

## Текущий статус
- Этап 0 (2026-11-09): `rust-client` собирается под `x86_64-pc-windows-msvc` без патчей, 0 предупреждений. `cargo test`: 354 passed, 10 ignored (9 e2e требуют WSL-сервер, 1 soak). В апстриме уже есть `cfg(not(unix))`-заглушки: `FdReceiver` (tun.rs), `run_tun` (dispatcher.rs), parent monitor и panic hook (main.rs) — это точки патчей этапа 1.
- 2026-11-10: этапы 1–9 готовы по коду (автономная сессия, журнал — `D:\Claude main\work\csqtt-s1-impl\AUTONOMOUS-PLAN.md`). Вживую не проверены: коннект через службу (админ + сервер), капча и VK-логин, per-app, установщик (нет ISCC), деплой (нет бинарников сервера).
