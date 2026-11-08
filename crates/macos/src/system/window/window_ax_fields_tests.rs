use super::*;
use accessibility_sys::{
    AXValueCreate, kAXErrorAPIDisabled, kAXErrorAttributeUnsupported, kAXErrorCannotComplete,
    kAXErrorFailure, kAXErrorInvalidUIElement, kAXErrorNoValue, kAXErrorSuccess,
    kAXValueTypeAXError,
};
use agent_desktop_core::ErrorCode;

fn slot_error_item(error: i32) -> CFType {
    let value = unsafe {
        AXValueCreate(
            kAXValueTypeAXError,
            &error as *const _ as *const std::ffi::c_void,
        )
    };
    unsafe { CFType::wrap_under_create_rule(value as _) }
}

fn batch(items: &[CFType]) -> CFArray<CFType> {
    CFArray::from_CFTypes(items)
}

#[test]
fn batched_read_decodes_minimized_and_title() {
    let items = batch(&[
        CFBoolean::true_value().as_CFType(),
        CFString::new("AgentDesk Fixture").as_CFType(),
    ]);

    assert_eq!(
        decode_window_fields(&items),
        Ok(WindowAxFields {
            minimized: Some(true),
            title: Some("AgentDesk Fixture".into()),
        })
    );
}

#[test]
fn failed_or_empty_title_slot_reads_as_no_title() {
    for title in [
        slot_error_item(kAXErrorNoValue),
        slot_error_item(kAXErrorCannotComplete),
        CFString::new("").as_CFType(),
    ] {
        let items = batch(&[CFBoolean::false_value().as_CFType(), title]);

        assert_eq!(
            decode_window_fields(&items),
            Ok(WindowAxFields {
                minimized: Some(false),
                title: None,
            })
        );
    }
}

#[test]
fn minimized_slot_keeps_single_read_failure_semantics() {
    let absent = batch(&[
        slot_error_item(kAXErrorNoValue),
        CFString::new("Doc").as_CFType(),
    ]);
    let transport = batch(&[
        slot_error_item(kAXErrorCannotComplete),
        CFString::new("Doc").as_CFType(),
    ]);

    assert_eq!(decode_window_fields(&absent).unwrap().minimized, None);
    assert_eq!(
        decode_window_fields(&transport),
        Err(kAXErrorCannotComplete)
    );
}

#[test]
fn short_batch_is_malformed() {
    let items = batch(&[CFBoolean::true_value().as_CFType()]);

    assert_eq!(decode_window_fields(&items), Err(MALFORMED_AX_VALUE));
}

fn later() -> Instant {
    Instant::now() + std::time::Duration::from_secs(5)
}

#[test]
fn whole_call_failure_falls_back_to_separate_reads() {
    for error in [
        kAXErrorCannotComplete,
        kAXErrorInvalidUIElement,
        kAXErrorFailure,
        kAXErrorNoValue,
        kAXErrorAttributeUnsupported,
    ] {
        let mut separate_reads = 0;
        let fields = fields_from_batch(error, None, later(), || {
            separate_reads += 1;
            Ok(WindowAxFields {
                minimized: Some(true),
                title: None,
            })
        })
        .unwrap();

        assert_eq!(separate_reads, 1, "ax error {error}");
        assert_eq!(fields.minimized, Some(true));
    }
}

#[test]
fn separate_minimized_failure_keeps_its_single_read_error() {
    let error = fields_from_batch(kAXErrorCannotComplete, None, later(), || {
        Err(crate::tree::surface_read::map_error(
            MINIMIZED,
            kAXErrorInvalidUIElement,
        ))
    })
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(error.details.unwrap()["attribute"], MINIMIZED);
}

#[test]
fn disabled_api_fails_without_separate_reads() {
    let error = fields_from_batch(kAXErrorAPIDisabled, None, later(), || {
        panic!("a disabled API must not be read again")
    })
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::PermDenied);
}

#[test]
fn successful_batch_is_decoded_without_separate_reads() {
    let items = batch(&[
        CFBoolean::false_value().as_CFType(),
        CFString::new("Inbox").as_CFType(),
    ]);

    let fields = fields_from_batch(kAXErrorSuccess, Some(items), later(), || {
        panic!("a decoded batch must not be read again")
    })
    .unwrap();

    assert_eq!(fields.title.as_deref(), Some("Inbox"));
}
