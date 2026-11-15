use agent_desktop_core::{
    AdapterError, CURSOR_ARRIVAL_TIMEOUT_MS, CursorOverlayControl, CursorPhase, ErrorCode,
};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

pub(super) const MAX_INSTRUCTION_BYTES: usize = 4 * 1024;

/// What one control connection to an overlay endpoint established.
pub(super) enum Exchange {
    /// No overlay listens on the endpoint.
    Absent,
    /// The overlay took the control and, when one is expected, acknowledged it.
    Delivered,
    /// The control left this process, but the overlay ended the stream or let
    /// the budget run out without acknowledging it.
    Unanswered(std::io::Error),
}

pub(super) fn exchange(
    socket: &Path,
    control: &CursorOverlayControl,
    deadline: Instant,
) -> Result<Exchange, AdapterError> {
    let mut stream = match UnixStream::connect(socket) {
        Ok(stream) => stream,
        Err(error) if matches!(error.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused) => {
            return Ok(Exchange::Absent);
        }
        Err(error) => {
            return Err(
                AdapterError::internal("Could not connect to the macOS cursor overlay")
                    .with_platform_detail(error.to_string()),
            );
        }
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(deadline_elapsed("Cursor overlay control deadline elapsed"));
    }
    stream
        .set_write_timeout(Some(remaining.min(Duration::from_secs(1))))
        .map_err(|error| {
            AdapterError::internal("Could not bound the cursor overlay control write")
                .with_platform_detail(error.to_string())
        })?;
    let payload = serde_json::to_vec(control).map_err(|error| {
        AdapterError::internal("Could not encode macOS cursor overlay control")
            .with_platform_detail(error.to_string())
    })?;
    if payload.len() > MAX_INSTRUCTION_BYTES {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Cursor overlay instruction exceeds the transport limit",
        ));
    }
    if let Err(error) = stream.write_all(&payload) {
        return closed_early(control, error, "Could not send the cursor overlay control");
    }
    if !control.expects_acknowledgement() {
        return Ok(Exchange::Delivered);
    }
    if let Err(error) = stream.shutdown(std::net::Shutdown::Write) {
        return closed_early(control, error, "Could not finish the cursor overlay control");
    }
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(deadline_elapsed("Timed out stopping macOS cursor overlays"));
    }
    let budget = if control.is_travel() {
        remaining.min(Duration::from_millis(CURSOR_ARRIVAL_TIMEOUT_MS))
    } else {
        remaining
    };
    stream.set_read_timeout(Some(budget)).map_err(|error| {
        AdapterError::internal("Could not bound the cursor overlay acknowledgement")
            .with_platform_detail(error.to_string())
    })?;
    let mut acknowledgement = [0_u8; 1];
    Ok(match stream.read_exact(&mut acknowledgement) {
        Ok(()) => Exchange::Delivered,
        Err(error) => Exchange::Unanswered(error),
    })
}

/// The peer ended the stream: the overlay is exiting, died, or refused this
/// one connection.
pub(super) fn is_closed(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::UnexpectedEof
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::BrokenPipe
            | ErrorKind::NotConnected
    )
}

pub(super) fn is_timed_out(error: &std::io::Error) -> bool {
    matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

pub(super) fn deadline_elapsed(message: &str) -> AdapterError {
    AdapterError::new(ErrorCode::Timeout, message)
}

fn closed_early(
    control: &CursorOverlayControl,
    error: std::io::Error,
    message: &str,
) -> Result<Exchange, AdapterError> {
    if control.is_disable() && is_closed(&error) {
        return Ok(Exchange::Unanswered(error));
    }
    Err(AdapterError::internal(message).with_platform_detail(error.to_string()))
}

pub(super) fn unanswered_error(
    control: &CursorOverlayControl,
    error: &std::io::Error,
) -> AdapterError {
    let phase = control.instruction().map(|instruction| instruction.phase());
    let message = match phase {
        Some(CursorPhase::Travel) => {
            tracing::warn!(
                session = control.session_id(),
                agent = control.agent_id(),
                %error,
                "cursor overlay travel was not acknowledged; continuing without cursor confirmation"
            );
            "macOS cursor overlay did not confirm cursor arrival"
        }
        Some(CursorPhase::Effect) => {
            tracing::warn!(
                session = control.session_id(),
                agent = control.agent_id(),
                %error,
                "cursor overlay effect was not acknowledged; continuing without effect confirmation"
            );
            "macOS cursor overlay did not confirm cursor effect"
        }
        _ => "macOS cursor overlay did not acknowledge the control",
    };
    AdapterError::internal(message).with_platform_detail(error.to_string())
}
