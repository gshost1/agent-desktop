use super::*;

const FIRST_READ_BYTE_BUDGETS: &[(&str, usize)] =
    &[("desktop", 4096), ("linux", 4096), ("windows", 5120)];

fn main_guide(name: &str) -> String {
    get(GetArgs {
        name: name.into(),
        full: false,
        reference: None,
    })
    .expect("skill is served")["content"]
        .as_str()
        .expect("string")
        .replace("\r\n", "\n")
}

fn reference(name: &str, topic: &str) -> String {
    get(GetArgs {
        name: name.into(),
        full: false,
        reference: Some(topic.into()),
    })
    .unwrap_or_else(|err| panic!("{name} {topic} is served: {err}"))["content"]
        .as_str()
        .expect("string")
        .to_owned()
}

fn topics_named_in(guide: &str) -> Vec<String> {
    let section = guide
        .split("\n## Topics\n")
        .nth(1)
        .expect("the guide has a Topics section");
    section
        .lines()
        .filter_map(|line| line.strip_prefix("| `"))
        .filter_map(|rest| rest.split('`').next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_guides_read_before_the_first_action_stay_within_their_byte_budgets() {
    for (name, budget) in FIRST_READ_BYTE_BUDGETS {
        let bytes = main_guide(name).len();
        assert!(
            bytes <= *budget,
            "skills get {name} is {bytes} bytes, budget {budget}: every later model call re-reads it, \
             so move detail behind a topic instead of growing the guide"
        );
    }
}

#[test]
fn every_topic_a_short_guide_names_is_served() {
    for (name, _) in FIRST_READ_BYTE_BUDGETS {
        let topics = topics_named_in(&main_guide(name));
        assert!(topics.len() >= 2, "{name} names too few topics: {topics:?}");
        for topic in topics {
            assert!(
                !reference(name, &topic).is_empty(),
                "{name} names topic {topic}, which is empty"
            );
        }
    }
}

#[test]
fn the_core_guide_keeps_the_rules_an_agent_needs_before_its_first_action() {
    let guide = main_guide("desktop");
    for rule in [
        "-i --compact --lines",
        "list-apps",
        ".data.matches[0].ref_id",
        "disposition.retry",
        "`safe`",
        "delivered_verified",
        "Do not snapshot just to confirm",
        "-w \"role:text\"",
        "data.surfaces",
        "STALE_REF",
        "AMBIGUOUS_TARGET",
        "--headed",
        "never a fixed `sleep`",
        "skills get platform",
    ] {
        assert!(guide.contains(rule), "the core guide lost: {rule}");
    }
}

#[test]
fn detail_moved_out_of_the_short_guides_is_still_served() {
    let moved: &[(&str, &str, &[&str])] = &[
        (
            "desktop",
            "refs-and-retry",
            &[
                "identity_unproven",
                "capture_incomplete",
                "retained_object_stale",
                "saved_ref_ambiguous",
                "resolution_identity_unknown",
                "Only `not_delivered` is safe",
                "Never switch to headed mode",
            ],
        ),
        (
            "desktop",
            "round-trips",
            &["`$S:e1`", "splatting", "jq -e", "head -c"],
        ),
        (
            "linux",
            "capabilities",
            &[
                "## Capability Table",
                "grab_focus_rejected",
                "selection_index_unproven",
                "native_popup_requires_input",
                "window-closed",
                "data.rendered: false",
            ],
        ),
        (
            "windows",
            "first-contact",
            &["APP_UNRESPONSIVE", "ctrl+shift+esc", "verification_scope"],
        ),
    ];
    for (name, topic, phrases) in moved {
        let body = reference(name, topic);
        for phrase in *phrases {
            assert!(body.contains(phrase), "{name} {topic} lost: {phrase}");
        }
    }
}
