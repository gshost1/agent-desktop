use super::{GetArgs, get};

const FIRST_READ_LIMIT: usize = 28_000;
const ENVELOPE_BYTES: usize = 64;

fn printed_bytes(name: &str) -> usize {
    let data = get(GetArgs {
        name: name.into(),
        full: false,
        reference: None,
    })
    .expect("skill");
    serde_json::to_string(&data).expect("json").len() + ENVELOPE_BYTES
}

#[test]
fn the_desktop_and_macos_skills_fit_one_tool_read() {
    let total = printed_bytes("desktop") + printed_bytes("macos");

    assert!(
        total <= FIRST_READ_LIMIT,
        "skills get desktop + skills get platform print {total} bytes on macOS; an agent's shell tool shows about {FIRST_READ_LIMIT}, so move detail into a reference"
    );
}
