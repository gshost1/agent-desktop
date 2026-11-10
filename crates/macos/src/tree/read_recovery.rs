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
/// so only the value of the attempt that is returned leaves this function. A
/// value that arrives after the deadline is released with the timeout.
pub(crate) fn read_owned(
    deadline: Instant,
    mut observe: impl FnMut() -> (i32, core_foundation::base::CFTypeRef),
) -> (i32, core_foundation::base::CFTypeRef) {
    let mut answered: core_foundation::base::CFTypeRef = std::ptr::null();
    let result = read(deadline, || {
        let (error, value) = observe();
        if error == accessibility_sys::kAXErrorSuccess {
            answered = value;
            return Ok(());
        }
        release(value);
        Err(error)
    });
    match result {
        Ok(()) => (accessibility_sys::kAXErrorSuccess, answered),
        Err(error) => {
            release(answered);
            (error, std::ptr::null())
        }
    }
}

fn release(value: core_foundation::base::CFTypeRef) {
    if !value.is_null() {
        unsafe { core_foundation::base::CFRelease(value) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use accessibility_sys::{
        kAXErrorAPIDisabled, kAXErrorCannotComplete, kAXErrorFailure, kAXErrorInvalidUIElement,
        kAXErrorSuccess,
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

    #[test]
    fn a_value_that_arrives_after_the_deadline_is_released() {
        use core_foundation::base::{CFGetRetainCount, CFRetain, TCFType};
        let held: core_foundation::dictionary::CFMutableDictionary =
            core_foundation::dictionary::CFMutableDictionary::new();
        let reference = held.as_CFTypeRef();
        let before = unsafe { CFGetRetainCount(reference) };
        let deadline = Instant::now() + Duration::from_millis(5);
        let (error, value) = read_owned(deadline, || {
            std::thread::sleep(Duration::from_millis(10));
            (kAXErrorSuccess, unsafe { CFRetain(reference) })
        });
        assert_eq!(error, kAXErrorCannotComplete);
        assert!(value.is_null());
        assert_eq!(unsafe { CFGetRetainCount(reference) }, before);
    }
}
