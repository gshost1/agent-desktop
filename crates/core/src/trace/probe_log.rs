//! Throwaway pointer-focus probe log on the `pfo` tracing target. Enable with
//! `RUST_LOG=pfo=debug`; `PFO_POLL_FRONT=1` also polls the front window for up
//! to one second after each raise.

use serde_json::{Value, json};
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static START: OnceLock<Instant> = OnceLock::new();

pub(crate) fn probe_log(event: &str, fields: Value) {
    let t_us = START.get_or_init(Instant::now).elapsed().as_micros() as u64;
    let wall_us = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_micros() as u64)
        .unwrap_or(0);
    tracing::debug!(
        target: "pfo",
        "{}",
        json!({ "event": event, "t_us": t_us, "wall_us": wall_us, "pid": std::process::id(), "fields": fields })
    );
}

pub(crate) fn probe_since_us(started: Instant) -> u64 {
    started.elapsed().as_micros() as u64
}

pub(crate) fn probe_poll_front_enabled() -> bool {
    std::env::var_os("PFO_POLL_FRONT").is_some()
}

pub(crate) fn probe_window(window: &crate::WindowInfo) -> Value {
    json!({ "id": window.id, "pid": window.pid, "instance": window.process_instance })
}

pub(crate) fn probe_front(read: &Result<Option<crate::WindowInfo>, crate::AdapterError>) -> Value {
    match read {
        Ok(Some(window)) => json!({ "ok": probe_window(window) }),
        Ok(None) => json!({ "none": true }),
        Err(error) => probe_error(error),
    }
}

pub(crate) fn probe_error(error: &crate::AdapterError) -> Value {
    json!({ "err": { "code": error.code.as_str(), "message": error.message, "platform_detail": error.platform_detail, "details": error.details } })
}

pub(crate) fn probe_app_error(error: &crate::AppError) -> Value {
    match error {
        crate::AppError::Adapter(error) => probe_error(error),
        other => json!({ "err": { "code": other.code(), "message": other.to_string() } }),
    }
}
