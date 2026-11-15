use super::*;

const CORE_REFS: &[SkillRef] = &[
    SkillRef {
        rel_path: "references/shared.md",
        body: "core shared",
    },
    SkillRef {
        rel_path: "references/aliased.md",
        body: "core aliased",
    },
];

const OWN_REFS: &[SkillRef] = &[
    SkillRef {
        rel_path: "references/shared.md",
        body: "platform shared",
    },
    SkillRef {
        rel_path: "references/only-here.md",
        body: "platform only",
    },
];

const OTHER_REFS: &[SkillRef] = &[SkillRef {
    rel_path: "references/elsewhere.md",
    body: "other platform",
}];

const FIXTURE: &[Skill] = &[
    Skill {
        canonical: "agent-desktop-other",
        aliases: &["other"],
        summary: "",
        platform: Some("other"),
        main: "other main",
        refs: OTHER_REFS,
    },
    Skill {
        canonical: "agent-desktop-own",
        aliases: &["own"],
        summary: "",
        platform: Some("own"),
        main: "own main",
        refs: OWN_REFS,
    },
    Skill {
        canonical: "agent-desktop",
        aliases: &["desktop"],
        summary: "",
        platform: None,
        main: "core main",
        refs: CORE_REFS,
    },
    Skill {
        canonical: "agent-desktop-aliased",
        aliases: &["aliased"],
        summary: "",
        platform: None,
        main: "aliased skill main",
        refs: &[],
    },
];

fn resolved_body(name: &str) -> Option<&'static str> {
    resolve_name(FIXTURE, name, Some("own"))
        .map(|(skill, topic)| topic.map_or(skill.main, |r| r.body))
}

#[test]
fn a_skill_alias_wins_over_a_topic_of_the_same_name() {
    assert_eq!(resolved_body("aliased"), Some("aliased skill main"));
}

#[test]
fn a_core_topic_wins_over_a_platform_topic_of_the_same_name() {
    assert_eq!(resolved_body("shared"), Some("core shared"));
}

#[test]
fn a_platform_topic_resolves_only_for_this_platform() {
    assert_eq!(resolved_body("only-here"), Some("platform only"));
    assert_eq!(resolved_body("elsewhere"), None);
}

#[test]
fn an_unknown_name_says_topics_are_accepted_and_lists_them() {
    let err = get(GetArgs {
        name: "nope".into(),
        full: false,
        reference: None,
    })
    .expect_err("unknown");
    let text = format!("{err}");
    assert!(text.contains("Unknown skill or topic 'nope'"), "{text}");
    assert!(text.contains("round-trips"), "{text}");
}
