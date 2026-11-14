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
    let skill = match find_skill(&args.name) {
        Ok(skill) => skill,
        Err(unknown) if args.reference.is_none() => {
            let (skill, topic) = find_topic(&args.name).ok_or(unknown)?;
            return Ok(reference_body(skill, topic));
        }
        Err(unknown) => return Err(unknown),
    };

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

fn find_skill(name: &str) -> Result<&'static Skill, AppError> {
    let needle = name.trim();
    if needle.eq_ignore_ascii_case(PLATFORM_ALIAS) {
        return platform_skill();
    }
    SKILLS
        .iter()
        .find(|s| s.aliases.iter().any(|a| a.eq_ignore_ascii_case(needle)))
        .ok_or_else(|| {
            let known: Vec<&str> = SKILLS
                .iter()
                .flat_map(|s| s.aliases.iter().copied())
                .collect();
            AppError::invalid_input(format!(
                "Unknown skill '{name}'. Known: {}",
                known.join(", ")
            ))
        })
}

/// A bare topic resolves against the core skill's references first, then this
/// build's platform skill, so `skills get round-trips` and
/// `skills get capabilities` need no skill name.
fn find_topic(name: &str) -> Option<(&'static Skill, &'static SkillRef)> {
    let needle = name.trim();
    SKILLS
        .iter()
        .filter(|s| {
            s.canonical == CORE_SKILL || (s.platform.is_some() && s.platform == CURRENT_PLATFORM)
        })
        .find_map(|s| {
            s.refs
                .iter()
                .find(|r| matches_ref(r.rel_path, needle))
                .map(|r| (s, r))
        })
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
    let stem = file.strip_suffix(".md").unwrap_or(file);
    file.eq_ignore_ascii_case(query) || stem.eq_ignore_ascii_case(query)
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
