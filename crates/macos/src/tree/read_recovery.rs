use std::time::{Duration, Instant};

pub(crate) fn read<T>(
    deadline: Instant,
    mut observe: impl FnMut() -> Result<T, i32>,
) -> Result<T, i32> {
    for attempt in 0..3 {
        if Instant::now() >= deadline {
            return Err(accessibility_sys::kAXErrorCannotComplete);
        }
        let result = observe();
        if Instant::now() >= deadline {
            return Err(accessibility_sys::kAXErrorCannotComplete);
        }
        if !matches!(result, Err(accessibility_sys::kAXErrorCannotComplete)) || attempt == 2 {
            return result;
        }
        std::thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(1)),
        );
    }
    Err(accessibility_sys::kAXErrorCannotComplete)
}

/// Applies [`read`] to a native call that hands back an owned value beside its
/// error. A value returned with a failed attempt is released before the retry,
/// so only the value of the attempt that is returned leaves this function.
pub(crate) fn read_owned(
    deadline: Instant,
    mut observe: impl FnMut() -> (i32, core_foundation::base::CFTypeRef),
) -> (i32, core_foundation::base::CFTypeRef) {
    let result = read(deadline, || {
        let (error, value) = observe();
        if error == accessibility_sys::kAXErrorSuccess {
            return Ok(value);
        }
        if !value.is_null() {
            unsafe { core_foundation::base::CFRelease(value) };
        }
        Err(error)
    });
    match result {
        Ok(value) => (accessibility_sys::kAXErrorSuccess, value),
        Err(error) => (error, std::ptr::null()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use accessibility_sys::{
        kAXErrorAPIDisabled, kAXErrorCannotComplete, kAXErrorFailure, kAXErrorInvalidUIElement,
    };

    #[test]
    fn transient_read_returns_the_fresh_value() {
        let mut reads = 0;
        let value = read(Instant::now() + Duration::from_secs(60), || {
            reads += 1;
            if reads == 1 {
                Err(kAXErrorCannotComplete)
            } else {
                Ok("textfield")
            }
        });
        assert_eq!(value, Ok("textfield"));
        assert_eq!(reads, 2);
    }

    #[test]
    fn only_transport_failures_retry_and_attempts_are_bounded() {
        for (error, expected) in [
            (kAXErrorCannotComplete, 3),
            (kAXErrorFailure, 1),
            (kAXErrorInvalidUIElement, 1),
            (kAXErrorAPIDisabled, 1),
        ] {
            let mut calls = 0;
            let result = read::<()>(Instant::now() + Duration::from_secs(60), || {
                calls += 1;
                Err(error)
            });
            assert_eq!(result, Err(error));
            assert_eq!(calls, expected);
        }
        assert_eq!(
            read::<()>(Instant::now(), || panic!("expired read")),
            Err(kAXErrorCannotComplete)
        );
    }

    #[test]
    fn a_busy_batch_is_read_again_in_place_instead_of_failing_the_node() {
        let mut calls = 0;
        let (error, value) = read_owned(Instant::now() + Duration::from_secs(60), || {
            calls += 1;
            if calls == 1 {
                (kAXErrorCannotComplete, std::ptr::null())
            } else {
                (accessibility_sys::kAXErrorSuccess, std::ptr::null())
            }
        });
        assert_eq!(error, accessibility_sys::kAXErrorSuccess);
        assert!(value.is_null());
        assert_eq!(
            calls, 2,
            "one slice overrun must not leave the node unknown"
        );

        let mut terminal_calls = 0;
        let (error, _) = read_owned(Instant::now() + Duration::from_secs(60), || {
            terminal_calls += 1;
            (kAXErrorInvalidUIElement, std::ptr::null())
        });
        assert_eq!(error, kAXErrorInvalidUIElement);
        assert_eq!(terminal_calls, 1);
    }
}
