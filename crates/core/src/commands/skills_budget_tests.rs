use super::*;

const FIRST_READ_BYTE_BUDGETS: &[(&str, usize)] =
    &[("desktop", 4608), ("linux", 4096), ("windows", 5120)];

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
    let section = section.split("\n## ").next().unwrap_or(section);
    section
        .lines()
        .filter_map(|line| {
            line.strip_prefix("| `")
                .or_else(|| line.strip_prefix("- `"))
        })
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
        "with `&&`, never `;`",
        "`find` it",
        "children_count=N",
        "// .error",
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

#[test]
fn no_shipped_example_pipes_snapshot_lines_into_jq_without_an_error_fallback() {
    for skill in SKILLS {
        let bodies = std::iter::once(("SKILL.md", skill.main))
            .chain(skill.refs.iter().map(|r| (r.rel_path, r.body)));
        for (path, body) in bodies {
            assert!(
                !body.contains(".data.lines[]'"),
                "{}/{path} pipes `.data.lines[]` into jq with no `?` and no `// .error`: an error \
                 envelope then prints only `Cannot iterate over null` and the agent never sees its code",
                skill.canonical
            );
        }
    }
}

const BASH_TOOL_SAFE_BYTES: usize = 28_000;

fn printed_bytes(name: &str) -> usize {
    let data = get(GetArgs {
        name: name.into(),
        full: false,
        reference: None,
    })
    .expect("skill is served");
    let envelope = crate::response::output::Response::ok("skills", data);
    serde_json::to_string(&envelope)
        .expect("envelope serializes")
        .len()
        + 1
}

#[test]
fn desktop_and_platform_together_fit_one_bash_tool_result_on_every_os() {
    let desktop = printed_bytes("desktop");
    let platforms: Vec<&Skill> = SKILLS.iter().filter(|s| s.platform.is_some()).collect();
    assert!(platforms.len() >= 3, "every OS has a platform skill");
    for platform in platforms {
        let total = desktop + printed_bytes(platform.canonical);
        assert!(
            total <= BASH_TOOL_SAFE_BYTES,
            "`skills get desktop; skills get platform` prints {total} bytes on {}, over \
             {BASH_TOOL_SAFE_BYTES}: near 30,000 characters the agent's Bash tool replaces the output \
             with a short preview and the agent never reads most of the guide",
            platform.platform.unwrap_or_default()
        );
    }
}

#[test]
fn each_short_guide_names_its_topics_before_any_other_section() {
    for (name, _) in FIRST_READ_BYTE_BUDGETS {
        let guide = main_guide(name);
        let topics = guide.find("\n## Topics\n").expect("Topics section");
        let first_section = guide.find("\n## ").expect("a section");
        assert_eq!(topics, first_section, "{name} must list its topics first");
    }
}
