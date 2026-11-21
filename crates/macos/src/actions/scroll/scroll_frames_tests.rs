use accessibility_sys::{
    AXValueCreate, kAXErrorCannotComplete, kAXErrorFailure, kAXErrorNoValue,
    kAXErrorNotImplemented, kAXErrorSuccess, kAXValueTypeAXError, kAXValueTypeCGPoint,
    kAXValueTypeCGSize,
};
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    string::CFString,
};
use core_graphics::geometry::{CGPoint, CGSize};

use super::{decode, decode_call};

fn ax_value<T>(kind: u32, value: &T) -> CFType {
    let value = unsafe { AXValueCreate(kind, value as *const T as *const std::ffi::c_void) };
    unsafe { CFType::wrap_under_create_rule(value as _) }
}

fn slot_error(error: i32) -> CFType {
    ax_value(kAXValueTypeAXError, &error)
}

fn position() -> CFType {
    ax_value(kAXValueTypeCGPoint, &CGPoint::new(10.0, 20.0))
}

fn size() -> CFType {
    ax_value(kAXValueTypeCGSize, &CGSize::new(100.0, 30.0))
}

fn null() -> CFType {
    unsafe { CFType::wrap_under_get_rule(core_foundation_sys::base::kCFNull as _) }
}

fn values(items: &[CFType]) -> CFArray<CFType> {
    CFArray::from_CFTypes(items)
}

#[test]
fn a_batched_read_decodes_the_role_and_the_frame() {
    let read = decode(
        &values(&[CFString::new("AXGroup").as_CFType(), position(), size()]),
        true,
    )
    .unwrap();

    assert_eq!(read.role.as_deref(), Some("AXGroup"));
    let frame = read.frame.unwrap();
    assert_eq!(
        (frame.x, frame.y, frame.width, frame.height),
        (10.0, 20.0, 100.0, 30.0)
    );
}

#[test]
fn a_transient_failure_of_a_frame_slot_is_unreadable_not_an_absent_frame() {
    for error in [
        kAXErrorFailure,
        kAXErrorNotImplemented,
        kAXErrorCannotComplete,
    ] {
        assert!(decode(&values(&[slot_error(error), size()]), false).is_err());
        assert!(decode(&values(&[position(), slot_error(error)]), false).is_err());
    }
}

#[test]
fn a_frame_the_element_does_not_report_is_absent() {
    let read = decode(&values(&[slot_error(kAXErrorNoValue), size()]), false).unwrap();
    assert!(read.frame.is_none());
}

#[test]
fn a_null_slot_a_value_of_another_type_or_a_short_batch_is_unreadable() {
    assert!(decode(&values(&[null(), size()]), false).is_err());
    assert!(decode(&values(&[size(), size()]), false).is_err());
    assert!(decode(&values(&[CFString::new("x").as_CFType(), size()]), false).is_err());
    assert!(decode(&values(&[position()]), false).is_err());
}

#[test]
fn the_role_slot_keeps_the_shared_absence_rule() {
    let read = decode(
        &values(&[slot_error(kAXErrorFailure), position(), size()]),
        true,
    )
    .unwrap();
    assert!(read.role.is_none());
    assert!(read.frame.is_some());

    let busy = values(&[slot_error(kAXErrorCannotComplete), position(), size()]);
    assert!(decode(&busy, true).is_err());
}

#[test]
fn a_failed_call_or_a_missing_result_is_unreadable_unless_the_attributes_are_absent() {
    assert!(decode_call(kAXErrorFailure, None, false).is_err());
    assert!(decode_call(kAXErrorSuccess, None, false).is_err());
    let absent = decode_call(kAXErrorNoValue, None, true).unwrap();
    assert!(absent.role.is_none() && absent.frame.is_none());
}

#[test]
fn a_decoded_frame_that_fails_validation_is_unreadable_not_an_absent_frame() {
    let nan = ax_value(kAXValueTypeCGPoint, &CGPoint::new(f64::NAN, 20.0));
    let huge = ax_value(kAXValueTypeCGPoint, &CGPoint::new(1.0e300, 20.0));
    let negative = ax_value(kAXValueTypeCGSize, &CGSize::new(-1.0, 30.0));

    assert!(decode(&values(&[nan, size()]), false).is_err());
    assert!(decode(&values(&[huge, size()]), false).is_err());
    assert!(decode(&values(&[position(), negative]), false).is_err());
}
