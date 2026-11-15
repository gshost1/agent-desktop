use crate::AppError;
use catalog::{SKILLS, Skill, SkillRef};
use serde_json::{Value, json};

const PLATFORM_ALIAS: &str = "platform";
const CORE_SKILL: &str = "agent-desktop";

const CURRENT_PLATFORM: Option<&str> = if cfg!(target_os = "macos") {
    Some("macos")
} else if cfg!(target_os = "windows") {
    Some("windows")
} else if cfg!(target_os = "linux") {
    Some("linux")
} else {
    None
};

pub struct GetArgs {
    pub name: String,
    pub full: bool,
    pub reference: Option<String>,
}

pub fn list() -> Result<Value, AppError> {
    let entries: Vec<Value> = SKILLS
        .iter()
        .map(|s| {
            let mut entry = json!({
                "name": s.canonical,
                "aliases": s.aliases,
                "summary": s.summary,
                "references": s.refs.iter().map(|r| r.rel_path).collect::<Vec<_>>(),
            });
            if let Some(platform) = s.platform {
                entry["platform"] = json!(platform);
                entry["current"] = json!(CURRENT_PLATFORM == Some(platform));
            }
            entry
        })
        .collect();
    Ok(json!({ "skills": entries }))
}

pub fn get(args: GetArgs) -> Result<Value, AppError> {
    let (skill, topic) = if args.name.trim().eq_ignore_ascii_case(PLATFORM_ALIAS) {
        (platform_skill()?, None)
    } else {
        resolve_name(SKILLS, &args.name, CURRENT_PLATFORM)
            .ok_or_else(|| unknown_name(&args.name))?
    };
    if let Some(topic) = topic {
        if args.reference.is_some() {
            return Err(unknown_name(&args.name));
        }
        return Ok(reference_body(skill, topic));
    }

    if let Some(rel) = args.reference {
        let refs = skill.refs;
        let r = refs
            .iter()
            .find(|r| matches_ref(r.rel_path, &rel))
            .ok_or_else(|| {
                let available: Vec<&str> = refs.iter().map(|r| r.rel_path).collect();
                AppError::invalid_input(format!(
                    "Unknown reference '{rel}' for skill '{}'. Available: {}",
                    skill.canonical,
                    available.join(", ")
                ))
            })?;
        return Ok(reference_body(skill, r));
    }

    let content = if args.full {
        render_full(skill)
    } else {
        skill.main.to_string()
    };

    Ok(json!({
        "skill": skill.canonical,
        "full": args.full,
        "content": content,
    }))
}

pub fn path() -> Result<Value, AppError> {
    Ok(json!({
        "location": "embedded",
        "note": "Skills are compiled into this binary and are always version-matched. Run `agent-desktop skills get <name>` to print a skill, or redirect into a file to extract a copy.",
        "available": SKILLS.iter().map(|s| s.canonical).collect::<Vec<_>>(),
    }))
}

fn reference_body(skill: &Skill, reference: &SkillRef) -> Value {
    json!({
        "skill": skill.canonical,
        "reference": reference.rel_path,
        "content": reference.body,
    })
}

/// A name is a skill alias first. Otherwise it is a topic: a reference of the
/// core skill, then of the platform skill for `platform`, so
/// `skills get round-trips` and `skills get capabilities` need no skill name.
fn resolve_name(
    skills: &'static [Skill],
    name: &str,
    platform: Option<&str>,
) -> Option<(&'static Skill, Option<&'static SkillRef>)> {
    let needle = name.trim();
    if let Some(skill) = skills
        .iter()
        .find(|s| s.aliases.iter().any(|a| a.eq_ignore_ascii_case(needle)))
    {
        return Some((skill, None));
    }
    let core = skills.iter().filter(|s| s.canonical == CORE_SKILL);
    let own = skills
        .iter()
        .filter(|s| s.platform.is_some() && s.platform == platform);
    core.chain(own).find_map(|s| {
        s.refs
            .iter()
            .find(|r| matches_ref(r.rel_path, needle))
            .map(|r| (s, Some(r)))
    })
}

fn unknown_name(name: &str) -> AppError {
    let skills: Vec<&str> = SKILLS
        .iter()
        .flat_map(|s| s.aliases.iter().copied())
        .collect();
    let topics: Vec<&str> = SKILLS
        .iter()
        .filter(|s| {
            s.canonical == CORE_SKILL || (s.platform.is_some() && s.platform == CURRENT_PLATFORM)
        })
        .flat_map(|s| s.refs.iter().map(|r| topic_stem(r.rel_path)))
        .collect();
    AppError::invalid_input(format!(
        "Unknown skill or topic '{name}'. Skills: {}. Topics: {}",
        skills.join(", "),
        topics.join(", ")
    ))
}

fn topic_stem(rel_path: &str) -> &str {
    let file = rel_path.rsplit('/').next().unwrap_or(rel_path);
    file.strip_suffix(".md").unwrap_or(file)
}

fn platform_skill() -> Result<&'static Skill, AppError> {
    SKILLS
        .iter()
        .find(|s| s.platform.is_some() && s.platform == CURRENT_PLATFORM)
        .ok_or_else(|| {
            AppError::invalid_input_with_suggestion(
                "This build's operating system has no platform skill yet",
                "Use `agent-desktop skills get desktop`; it covers the loop on every OS",
            )
        })
}

fn matches_ref(rel_path: &str, query: &str) -> bool {
    if rel_path.eq_ignore_ascii_case(query) {
        return true;
    }
    let file = rel_path.rsplit('/').next().unwrap_or(rel_path);
    file.eq_ignore_ascii_case(query) || topic_stem(rel_path).eq_ignore_ascii_case(query)
}

fn render_full(skill: &Skill) -> String {
    let refs = skill.refs;
    let mut out = String::with_capacity(
        skill.main.len() + refs.iter().map(|r| r.body.len() + 64).sum::<usize>(),
    );
    out.push_str(skill.main);
    for r in refs {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("\n--- ");
        out.push_str(r.rel_path);
        out.push_str(" ---\n\n");
        out.push_str(r.body);
    }
    out
}

#[path = "skills_catalog.rs"]
mod catalog;

#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "skills_budget_tests.rs"]
mod budget_tests;

#[cfg(test)]
#[path = "skills_resolve_tests.rs"]
mod resolve_tests;

#[cfg(test)]
#[path = "skills_guidance_tests.rs"]
mod guidance_tests;
