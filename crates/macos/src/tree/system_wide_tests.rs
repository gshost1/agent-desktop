use super::*;
use core_foundation::base::TCFType;

const CHILD: &str = "AGENT_DESKTOP_SYSTEM_WIDE_FRESH_PROCESS";
const FIRST_READ: &str = "first-system-wide-read=";
const UNTRUSTED: &str = "system-wide-read-skipped=untrusted";

#[test]
#[ignore = "runs only as the fresh process started by the test below"]
fn fresh_process_reads_the_focused_application() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    if !unsafe { accessibility_sys::AXIsProcessTrusted() } {
        println!("{UNTRUSTED}");
        return;
    }
    let system = element();
    let attribute = core_foundation::string::CFString::from_static_string("AXFocusedApplication");
    let mut value: core_foundation_sys::base::CFTypeRef = std::ptr::null();
    let error = unsafe {
        accessibility_sys::AXUIElementCopyAttributeValue(
            system.0,
            attribute.as_concrete_TypeRef(),
            &mut value,
        )
    };
    if !value.is_null() {
        unsafe { core_foundation::base::CFRelease(value) };
    }
    println!("{FIRST_READ}{error}");
}

#[test]
fn the_first_system_wide_read_of_a_fresh_process_is_not_refused() {
    let Ok(test_binary) = std::env::current_exe() else {
        panic!("the test binary path is unreadable");
    };
    let output = std::process::Command::new(test_binary)
        .args([
            "--exact",
            "tree::system_wide::tests::fresh_process_reads_the_focused_application",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, "1")
        .output();
    let Ok(output) = output else {
        panic!("the fresh test process did not start");
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.lines().any(|line| line.trim() == UNTRUSTED) {
        eprintln!("skipped: the fresh test process is not trusted for accessibility");
        return;
    }
    let first_read = stdout
        .lines()
        .find_map(|line| line.split(FIRST_READ).nth(1))
        .and_then(|code| code.trim().parse::<i32>().ok());
    let Some(first_read) = first_read else {
        panic!("the fresh test process reported no read: {stdout}");
    };
    assert!(
        first_read == accessibility_sys::kAXErrorSuccess
            || first_read == accessibility_sys::kAXErrorNoValue,
        "a trusted fresh process read AXFocusedApplication with error {first_read}"
    );
}
