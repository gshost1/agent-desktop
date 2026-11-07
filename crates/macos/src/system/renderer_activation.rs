use crate::tree::renderer_probe;
use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, ProcessIdentity};
use std::time::Duration;

const ENHANCED_ACTIVATION_DELAY: Duration = Duration::from_secs(2);

pub(crate) fn activate(process: ProcessIdentity, deadline: Deadline) -> Result<(), AdapterError> {
    let identity =
        crate::system::process::process_identity::require_core(&process).map_err(not_delivered)?;
    let application = crate::tree::element_for_pid(identity.pid());
    let attribute = renderer_probe::activation_attribute(
        &application,
        instant(deadline).map_err(not_delivered)?,
    )
    .map_err(not_delivered)?
    .ok_or_else(|| {
        not_delivered(AdapterError::not_supported(
            "renderer accessibility activation",
        ))
    })?;
    let current = crate::tree::surface_read::boolean(
        &application,
        attribute,
        instant(deadline).map_err(not_delivered)?,
    )
    .map_err(not_delivered)?;
    if current == Some(true) {
        return Ok(());
    }
    let chromium = attribute == renderer_probe::ENHANCED
        && crate::system::renderer_kind::detect_chromium(identity.pid()).is_some();
    enable_attribute(
        &application,
        attribute,
        settle_delay(attribute, chromium),
        deadline,
    )
}

/// How long to wait after switching accessibility on before the first read.
/// A Chromium renderer reads `AXEnhancedUserInterface` back as on at once but
/// switches to its complete mode only after a debounce; until then its tree
/// reads with inactive actions, so it gets a fixed settle delay. Every Cocoa
/// app exposes that attribute too, but a native app's tree does not depend on
/// it, so a native app does not pay the delay.
fn settle_delay(attribute: &str, chromium: bool) -> Duration {
    if attribute == renderer_probe::ENHANCED && chromium {
        ENHANCED_ACTIVATION_DELAY
    } else {
        Duration::ZERO
    }
}

fn enable_attribute(
    application: &crate::tree::AXElement,
    attribute_name: &str,
    delay: Duration,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};

    if deadline.remaining() <= delay {
        return Err(not_delivered(deadline.timeout_error()));
    }
    let attribute = CFString::new(attribute_name);
    let error = crate::tree::properties::ax_ipc::set_attribute_value(
        application,
        attribute.as_concrete_TypeRef(),
        CFBoolean::true_value().as_CFTypeRef(),
        deadline,
    )?;
    let readback = crate::tree::surface_read::boolean(
        application,
        attribute_name,
        instant(deadline).map_err(uncertain)?,
    )
    .map_err(uncertain)?;
    let delivered = activation_delivered(application, attribute_name, error, readback)?;
    if !delivered {
        return Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "Renderer did not enable accessibility activation",
        )
        .with_disposition(DeliverySemantics::not_delivered()));
    }
    std::thread::sleep(deadline.remaining().min(delay));
    if deadline.is_expired() {
        return Err(deadline
            .timeout_error()
            .with_disposition(DeliverySemantics::delivered_unverified()));
    }
    Ok(())
}

fn activation_delivered(
    application: &crate::tree::AXElement,
    attribute: &str,
    error: i32,
    readback: Option<bool>,
) -> Result<bool, AdapterError> {
    if readback == Some(true) {
        return Ok(true);
    }
    let delivered = crate::actions::ax_mutation::classify_result(
        application,
        attribute,
        "AXUIElementSetAttributeValue",
        error,
    )?;
    if delivered && readback == Some(false) && attribute == renderer_probe::ENHANCED {
        return Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "Renderer accessibility activation was not reflected in the attribute value",
        )
        .with_disposition(DeliverySemantics::delivered_unverified()));
    }
    Ok(delivered)
}

fn uncertain(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::uncertain())
}

fn instant(deadline: Deadline) -> Result<std::time::Instant, AdapterError> {
    std::time::Instant::now()
        .checked_add(deadline.remaining_slice(std::time::Duration::from_millis(250))?)
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Deadline is out of range"))
}

fn not_delivered(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_chromium_renderer_pays_the_enhanced_settle_delay() {
        assert_eq!(
            settle_delay(renderer_probe::ENHANCED, true),
            ENHANCED_ACTIVATION_DELAY
        );
        assert_eq!(
            settle_delay(renderer_probe::ENHANCED, false),
            Duration::ZERO
        );
        assert_eq!(settle_delay(renderer_probe::MANUAL, true), Duration::ZERO);
        assert_eq!(settle_delay(renderer_probe::MANUAL, false), Duration::ZERO);
    }

    #[test]
    fn observed_activation_overrides_a_setters_not_implemented_response() {
        let application = crate::tree::AXElement(std::ptr::null_mut());
        assert!(
            activation_delivered(
                &application,
                renderer_probe::ENHANCED,
                accessibility_sys::kAXErrorNotImplemented,
                Some(true),
            )
            .unwrap()
        );
        assert!(
            !activation_delivered(
                &application,
                renderer_probe::ENHANCED,
                accessibility_sys::kAXErrorNotImplemented,
                Some(false),
            )
            .unwrap()
        );
    }

    #[test]
    fn enhanced_activation_with_insufficient_budget_never_touches_native_handle() {
        let application = crate::tree::AXElement(std::ptr::null_mut());
        let deadline = Deadline::after(1_000).unwrap();
        let error = enable_attribute(
            &application,
            renderer_probe::ENHANCED,
            ENHANCED_ACTIVATION_DELAY,
            deadline,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Timeout);
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    }

    #[test]
    fn reported_success_with_disabled_readback_is_not_activation_success() {
        let application = crate::tree::AXElement(std::ptr::null_mut());
        let error = activation_delivered(
            &application,
            renderer_probe::ENHANCED,
            accessibility_sys::kAXErrorSuccess,
            Some(false),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ActionFailed);
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    }

    #[test]
    fn accepted_manual_write_with_disabled_readback_is_not_an_error() {
        let application = crate::tree::AXElement(std::ptr::null_mut());
        assert!(
            activation_delivered(
                &application,
                renderer_probe::MANUAL,
                accessibility_sys::kAXErrorSuccess,
                Some(false),
            )
            .unwrap()
        );
    }

    #[test]
    fn missing_readback_keeps_legacy_success_and_preserves_real_failures() {
        let application = crate::tree::AXElement(std::ptr::null_mut());
        assert!(
            activation_delivered(
                &application,
                renderer_probe::MANUAL,
                accessibility_sys::kAXErrorSuccess,
                None,
            )
            .unwrap()
        );
        let error = activation_delivered(
            &application,
            renderer_probe::ENHANCED,
            accessibility_sys::kAXErrorCannotComplete,
            None,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::AppUnresponsive);
        assert_eq!(error.disposition, DeliverySemantics::uncertain());
    }
}
