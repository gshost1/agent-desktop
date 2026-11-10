use super::decode;
use accessibility_sys::{
    AXValueCreate, kAXErrorCannotComplete, kAXErrorNoValue, kAXValueTypeAXError,
};
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    boolean::CFBoolean,
    string::CFString,
};

fn slot_error(error: i32) -> CFType {
    let value = unsafe {
        AXValueCreate(
            kAXValueTypeAXError,
            &error as *const _ as *const std::ffi::c_void,
        )
    };
    unsafe { CFType::wrap_under_create_rule(value as _) }
}

fn values(items: &[CFType]) -> CFArray<CFType> {
    CFArray::from_CFTypes(items)
}

#[test]
fn batched_read_decodes_title_and_minimized_state() {
    let batch = values(&[
        CFString::new("Quarterly").as_CFType(),
        CFBoolean::true_value().as_CFType(),
    ]);

    assert_eq!(
        decode(&batch).unwrap(),
        (Some("Quarterly".to_owned()), Some(true))
    );
}

#[test]
fn a_failed_title_slot_leaves_the_title_unknown_without_failing() {
    let batch = values(&[
        slot_error(kAXErrorCannotComplete),
        CFBoolean::false_value().as_CFType(),
    ]);

    assert_eq!(decode(&batch).unwrap(), (None, Some(false)));
}

#[test]
fn a_busy_minimized_slot_still_fails_like_the_single_read() {
    let batch = values(&[
        CFString::new("Quarterly").as_CFType(),
        slot_error(kAXErrorCannotComplete),
    ]);

    let error = decode(&batch).unwrap_err();

    assert_eq!(error.code, agent_desktop_core::ErrorCode::Timeout);
}

#[test]
fn absent_slots_are_unknown_values() {
    let batch = values(&[slot_error(kAXErrorNoValue), slot_error(kAXErrorNoValue)]);

    assert_eq!(decode(&batch).unwrap(), (None, None));
}
