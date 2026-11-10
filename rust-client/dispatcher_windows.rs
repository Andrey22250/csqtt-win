use crate::{
    packet::{PacketBuf, PacketPool},
    stats::Stats,
};
use std::{
    sync::{
        Arc,
        atomic::Ordering,
    },
    time::Duration,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const TUN_RX_BURST: usize = 32;
const TUN_TX_BURST: usize = 128;
const TUN_REPLACEMENT_TIMEOUT: Duration = Duration::from_secs(20);
const TUN_RETRY_TOLERANCE: u32 = 40;
const TUN_RETRY_BACKOFF_STEP_MS: u64 = 25;
const TUN_RETRY_BACKOFF_MAX_MS: u64 = 400;
const TUN_RETRY_BACKOFF_STEPS: u32 = 16;

fn tun_retry_backoff(streak: u32) -> Duration {
    let factor = u64::from(streak.clamp(1, TUN_RETRY_BACKOFF_STEPS));
    Duration::from_millis((TUN_RETRY_BACKOFF_STEP_MS * factor).min(TUN_RETRY_BACKOFF_MAX_MS))
}

fn log_dropped_packet(counter: &mut u64, reason: &str) {
    *counter += 1;
    if *counter <= 8 || (*counter % 1000 == 0) {
        crate::log_error!("[ОШИБКА] Запись TUN: пакет #{counter} отброшен ({reason})");
    }
}

fn fail_on_panic(
    dispatcher: &super::Dispatcher,
    name: &str,
    result: Result<(), tokio::task::JoinError>,
) {
    if let Err(error) = result
        && error.is_panic()
    {
        crate::log_error!("[СУПЕРВИЗОР] {name} завершился аварийно: {error}");
        dispatcher.terminate(super::Termination::Fatal);
    }
}

impl super::Dispatcher {
    pub(super) async fn run_tun(
        self: &Arc<Self>,
        session: crate::tun::TunSession,
        pool: Arc<PacketPool>,
        stats: Arc<Stats>,
        replacements: &mut mpsc::Receiver<crate::tun::TunSession>,
    ) {
        let mut current_session = session;
        loop {
            let generation_cancel = self.cancel.child_token();

            let read_dispatcher = self.clone();
            let read_cancel = generation_cancel.clone();
            let read_session = current_session.session.clone();
            let read_pool = pool.clone();
            let read_stats = stats.clone();
            let mut read_task = tokio::spawn(async move {
                read_dispatcher
                    .read_tun(read_session, read_pool, read_stats, read_cancel)
                    .await;
            });

            let write_dispatcher = self.clone();
            let write_cancel = generation_cancel.clone();
            let write_session = current_session.session.clone();
            let write_stats = stats.clone();
            let mut write_task = tokio::spawn(async move {
                write_dispatcher
                    .write_tun(write_session, write_stats, write_cancel)
                    .await;
            });

            let received = tokio::select! {
                _ = self.cancel.cancelled() => None,
                replacement = replacements.recv() => replacement,
                result = &mut read_task => {
                    fail_on_panic(self, "TUN reader", result);
                    None
                }
                result = &mut write_task => {
                    fail_on_panic(self, "TUN writer", result);
                    None
                }
            };

            generation_cancel.cancel();
            let _ = current_session.session.shutdown();

            if !read_task.is_finished() {
                let _ = read_task.await;
            }
            if !write_task.is_finished() {
                let _ = write_task.await;
            }

            if self.cancel.is_cancelled() {
                return;
            }

            let next = match received {
                Some(session) => session,
                None => {
                    crate::log_error!("[TUN] I/O сессия завершилась, ожидаем новый TUN FD...");
                    self.tun_io_lost.notify_one();
                    let replacement = tokio::select! {
                        _ = self.cancel.cancelled() => return,
                        replacement = tokio::time::timeout(TUN_REPLACEMENT_TIMEOUT, replacements.recv()) => replacement,
                    };
                    match replacement {
                        Ok(Some(session)) => session,
                        Ok(None) => {
                            crate::log_error!("[TUN] Канал замены TUN FD закрыт, TUN потерян");
                            self.terminate(super::Termination::TunLost);
                            return;
                        }
                        Err(_) => {
                            crate::log_error!(
                                "[TUN] Новый TUN FD не получен за {} с, TUN потерян",
                                TUN_REPLACEMENT_TIMEOUT.as_secs()
                            );
                            self.terminate(super::Termination::TunLost);
                            return;
                        }
                    }
                }
            };

            crate::log_error!("[КЛИЕНТ] TUN FD заменён без перезапуска потоков");
            current_session = next;
        }
    }

    async fn read_tun(
        self: Arc<Self>,
        session: Arc<wintun::Session>,
        pool: Arc<PacketPool>,
        stats: Arc<Stats>,
        cancel: CancellationToken,
    ) {
        let (packet_tx, mut packet_rx) = mpsc::channel::<PacketBuf>(TUN_RX_BURST);

        let reader_dispatcher = self.clone();
        let reader_session = session.clone();
        let reader_pool = pool.clone();
        let reader_stats = stats.clone();
        let reader_cancel = cancel.clone();
        let handle = tokio::runtime::Handle::current();

        let reader_thread = match std::thread::Builder::new()
            .name("tun-reader".to_string())
            .spawn(move || {
                while !reader_cancel.is_cancelled() {
                    let wintun_pkt = match reader_session.receive_blocking() {
                        Ok(p) => p,
                        Err(wintun::Error::ShuttingDown) => break,
                        Err(err) => {
                            if reader_cancel.is_cancelled() {
                                break;
                            }
                            crate::log_error!("[TUN] Чтение TUN: ошибка Wintun: {err}");
                            break;
                        }
                    };

                    let bytes = wintun_pkt.bytes();
                    let length = bytes.len();
                    if !Self::is_forwardable_packet(bytes) {
                        drop(wintun_pkt);
                        continue;
                    }
                    if reader_dispatcher.workers.load().is_empty() {
                        drop(wintun_pkt);
                        continue;
                    }

                    let mut packet = loop {
                        if let Some(p) = reader_pool.try_acquire() {
                            break p;
                        }
                        if reader_cancel.is_cancelled() {
                            return;
                        }
                        let cancelled = handle.block_on(async {
                            tokio::select! {
                                biased;
                                _ = reader_cancel.cancelled() => true,
                                _ = reader_pool.wait_available() => false,
                            }
                        });
                        if cancelled {
                            return;
                        }
                    };

                    let area = packet.read_area();
                    if length > area.len() {
                        drop(packet);
                        drop(wintun_pkt);
                        continue;
                    }
                    area[..length].copy_from_slice(bytes);
                    if packet.set_read_len(length).is_err() {
                        drop(packet);
                        drop(wintun_pkt);
                        continue;
                    }
                    drop(wintun_pkt);

                    reader_stats
                        .total_bytes_up
                        .fetch_add(length as i64, Ordering::Relaxed);

                    if packet_tx.blocking_send(packet).is_err() {
                        break;
                    }
                }
            }) {
            Ok(handle) => Some(handle),
            Err(e) => {
                crate::log_error!("[ОШИБКА] Не удалось запустить поток TUN reader: {e}");
                return;
            }
        };

        let session_shutdown = session.clone();
        let shutdown_cancel = cancel.clone();
        let shutdown_watcher = tokio::spawn(async move {
            shutdown_cancel.cancelled().await;
            let _ = session_shutdown.shutdown();
        });

        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break,
                packet = packet_rx.recv() => match packet {
                    Some(packet) => {
                        self.dispatch(packet).await;
                    }
                    None => break,
                }
            }
        }

        // Drop the receiver first: a reader blocked in blocking_send must not deadlock join().
        drop(packet_rx);
        let _ = session.shutdown();
        shutdown_watcher.abort();
        if let Some(handle) = reader_thread {
            if let Err(panic_payload) = handle.join() {
                std::panic::resume_unwind(panic_payload);
            }
        }
    }

    async fn write_tun(
        &self,
        session: Arc<wintun::Session>,
        stats: Arc<Stats>,
        cancel: CancellationToken,
    ) {
        let mut pending: Option<PacketBuf> = None;
        let mut dropped = 0u64;
        let mut error_streak = 0u32;

        loop {
            if cancel.is_cancelled() {
                return;
            }

            if pending.is_none() {
                if let Some(mut packet) = self.pop_inbound() {
                    super::strip_legacy_downlink_prefix(&mut packet);
                    if packet.len() >= 20 && Self::is_forwardable_packet(packet.as_slice()) {
                        pending = Some(packet);
                    }
                } else {
                    tokio::select! {
                        biased;
                        _ = cancel.cancelled() => return,
                        _ = self.tun_wake.notified() => {},
                    }
                    continue;
                }
            }

            let mut burst = 0usize;
            let mut retry_delay = None;

            while burst < TUN_TX_BURST {
                let Some(packet) = pending.as_ref() else {
                    break;
                };

                let len = packet.len();
                if len > u16::MAX as usize {
                    log_dropped_packet(&mut dropped, "размер пакета превышает лимит Wintun");
                    pending = None;
                    break;
                }

                match session.allocate_send_packet(len as u16) {
                    Ok(mut wintun_packet) => {
                        wintun_packet.bytes_mut().copy_from_slice(packet.as_slice());
                        session.send_packet(wintun_packet);

                        dropped = 0;
                        error_streak = 0;
                        stats
                            .total_bytes_down
                            .fetch_add(len as i64, Ordering::Relaxed);
                        burst += 1;

                        if let Some(mut next_packet) = self.pop_inbound() {
                            super::strip_legacy_downlink_prefix(&mut next_packet);
                            if next_packet.len() >= 20
                                && Self::is_forwardable_packet(next_packet.as_slice())
                            {
                                pending = Some(next_packet);
                            } else {
                                pending = None;
                                break;
                            }
                        } else {
                            pending = None;
                            break;
                        }
                    }
                    Err(error) => {
                        error_streak += 1;
                        if error_streak >= TUN_RETRY_TOLERANCE {
                            crate::log_error!(
                                "[TUN] Запись TUN: интерфейс не отвечает ({error}) после {error_streak} попыток, переподнимаем"
                            );
                            return;
                        }
                        log_dropped_packet(&mut dropped, &error.to_string());
                        retry_delay = Some(tun_retry_backoff(error_streak));
                        break;
                    }
                }
            }

            if burst >= TUN_TX_BURST {
                tokio::task::yield_now().await;
            }

            if let Some(delay) = retry_delay {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(delay) => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tun_retry_backoff() {
        assert_eq!(tun_retry_backoff(0), Duration::from_millis(25));
        assert_eq!(tun_retry_backoff(1), Duration::from_millis(25));
        assert_eq!(tun_retry_backoff(2), Duration::from_millis(50));
        assert_eq!(tun_retry_backoff(4), Duration::from_millis(100));
        assert_eq!(tun_retry_backoff(16), Duration::from_millis(400));
        assert_eq!(tun_retry_backoff(50), Duration::from_millis(400));
    }

    #[test]
    fn test_log_dropped_packet_counter() {
        let mut counter = 0u64;
        for _ in 0..10 {
            log_dropped_packet(&mut counter, "test drop");
        }
        assert_eq!(counter, 10);
    }
}
