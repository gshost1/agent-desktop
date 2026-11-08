use super::*;
use accessibility_sys::{
    AXValueCreate, kAXErrorCannotComplete, kAXErrorNoValue, kAXValueTypeAXError,
};

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
