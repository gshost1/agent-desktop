//! The skill files compiled into the binary and the table `skills` serves them from.

const SKILL_DESKTOP_MAIN: &str = include_str!("../../../../skills/agent-desktop/SKILL.md");
const SKILL_DESKTOP_REF_OBSERVATION: &str =
    include_str!("../../../../skills/agent-desktop/references/commands-observation.md");
const SKILL_DESKTOP_REF_INTERACTION: &str =
    include_str!("../../../../skills/agent-desktop/references/commands-interaction.md");
const SKILL_DESKTOP_REF_SYSTEM: &str =
    include_str!("../../../../skills/agent-desktop/references/commands-system.md");
pub(super) const SKILL_DESKTOP_REF_WORKFLOWS: &str =
    include_str!("../../../../skills/agent-desktop/references/workflows.md");
const SKILL_DESKTOP_REF_REFS_AND_RETRY: &str =
    include_str!("../../../../skills/agent-desktop/references/refs-and-retry.md");
const SKILL_DESKTOP_REF_ROUND_TRIPS: &str =
    include_str!("../../../../skills/agent-desktop/references/round-trips.md");

const SKILL_MACOS_MAIN: &str = include_str!("../../../../skills/agent-desktop-macos/SKILL.md");
const SKILL_MACOS_REF_TROUBLESHOOTING: &str =
    include_str!("../../../../skills/agent-desktop-macos/references/troubleshooting.md");

const SKILL_WINDOWS_MAIN: &str = include_str!("../../../../skills/agent-desktop-windows/SKILL.md");
const SKILL_WINDOWS_REF_PERMISSIONS: &str = include_str!(
    "../../../../skills/agent-desktop-windows/references/permissions-and-elevation.md"
);
const SKILL_WINDOWS_REF_CHROMIUM: &str =
    include_str!("../../../../skills/agent-desktop-windows/references/chromium-and-electron.md");
const SKILL_WINDOWS_REF_TROUBLESHOOTING: &str =
    include_str!("../../../../skills/agent-desktop-windows/references/troubleshooting.md");
const SKILL_WINDOWS_REF_SHELL: &str =
    include_str!("../../../../skills/agent-desktop-windows/references/shell-and-overlay.md");
const SKILL_WINDOWS_REF_FIRST_CONTACT: &str =
    include_str!("../../../../skills/agent-desktop-windows/references/first-contact.md");

const SKILL_LINUX_MAIN: &str = include_str!("../../../../skills/agent-desktop-linux/SKILL.md");
const SKILL_LINUX_REF_CAPABILITIES: &str =
    include_str!("../../../../skills/agent-desktop-linux/references/capabilities.md");
const SKILL_LINUX_REF_BUS: &str =
    include_str!("../../../../skills/agent-desktop-linux/references/bus-and-permissions.md");
const SKILL_LINUX_REF_DISPLAY: &str =
    include_str!("../../../../skills/agent-desktop-linux/references/display-servers-and-input.md");
const SKILL_LINUX_REF_WINDOWS: &str =
    include_str!("../../../../skills/agent-desktop-linux/references/windows-and-lifecycle.md");
const SKILL_LINUX_REF_CAPTURE: &str =
    include_str!("../../../../skills/agent-desktop-linux/references/capture-and-clipboard.md");
const SKILL_LINUX_REF_SHELL: &str = include_str!(
    "../../../../skills/agent-desktop-linux/references/shell-tray-and-notifications.md"
);
const SKILL_LINUX_REF_TROUBLESHOOTING: &str =
    include_str!("../../../../skills/agent-desktop-linux/references/troubleshooting.md");

const SKILL_WEB_MAIN: &str = include_str!("../../../../skills/agent-desktop-web/SKILL.md");
const SKILL_WEB_REF_REFERENCE: &str =
    include_str!("../../../../skills/agent-desktop-web/references/reference.md");

const SKILL_JEV_MAIN: &str = include_str!("../../../../skills/jev-desktop/SKILL.md");

const SKILL_FFI_MAIN: &str = include_str!("../../../../skills/agent-desktop-ffi/SKILL.md");
const SKILL_FFI_REF_BUILD: &str =
    include_str!("../../../../skills/agent-desktop-ffi/references/build-and-link.md");
const SKILL_FFI_REF_ERRORS: &str =
    include_str!("../../../../skills/agent-desktop-ffi/references/error-handling.md");
const SKILL_FFI_REF_OWNERSHIP: &str =
    include_str!("../../../../skills/agent-desktop-ffi/references/ownership.md");
const SKILL_FFI_REF_THREADING: &str =
    include_str!("../../../../skills/agent-desktop-ffi/references/threading.md");

pub(super) struct SkillRef {
    pub(super) rel_path: &'static str,
    pub(super) body: &'static str,
}

pub(super) struct Skill {
    pub(super) canonical: &'static str,
    pub(super) aliases: &'static [&'static str],
    pub(super) summary: &'static str,
    pub(super) platform: Option<&'static str>,
    pub(super) main: &'static str,
    pub(super) refs: &'static [SkillRef],
}

const SKILL_DESKTOP_REFS: &[SkillRef] = &[
    SkillRef {
        rel_path: "references/refs-and-retry.md",
        body: SKILL_DESKTOP_REF_REFS_AND_RETRY,
    },
    SkillRef {
        rel_path: "references/round-trips.md",
        body: SKILL_DESKTOP_REF_ROUND_TRIPS,
    },
    SkillRef {
        rel_path: "references/commands-observation.md",
        body: SKILL_DESKTOP_REF_OBSERVATION,
    },
    SkillRef {
        rel_path: "references/commands-interaction.md",
        body: SKILL_DESKTOP_REF_INTERACTION,
    },
    SkillRef {
        rel_path: "references/commands-system.md",
        body: SKILL_DESKTOP_REF_SYSTEM,
    },
    SkillRef {
        rel_path: "references/workflows.md",
        body: SKILL_DESKTOP_REF_WORKFLOWS,
    },
];

const SKILL_MACOS_REFS: &[SkillRef] = &[SkillRef {
    rel_path: "references/troubleshooting.md",
    body: SKILL_MACOS_REF_TROUBLESHOOTING,
}];

const SKILL_FFI_REFS: &[SkillRef] = &[
    SkillRef {
        rel_path: "references/build-and-link.md",
        body: SKILL_FFI_REF_BUILD,
    },
    SkillRef {
        rel_path: "references/error-handling.md",
        body: SKILL_FFI_REF_ERRORS,
    },
    SkillRef {
        rel_path: "references/ownership.md",
        body: SKILL_FFI_REF_OWNERSHIP,
    },
    SkillRef {
        rel_path: "references/threading.md",
        body: SKILL_FFI_REF_THREADING,
    },
];

const SKILL_WINDOWS_REFS: &[SkillRef] = &[
    SkillRef {
        rel_path: "references/first-contact.md",
        body: SKILL_WINDOWS_REF_FIRST_CONTACT,
    },
    SkillRef {
        rel_path: "references/permissions-and-elevation.md",
        body: SKILL_WINDOWS_REF_PERMISSIONS,
    },
    SkillRef {
        rel_path: "references/chromium-and-electron.md",
        body: SKILL_WINDOWS_REF_CHROMIUM,
    },
    SkillRef {
        rel_path: "references/troubleshooting.md",
        body: SKILL_WINDOWS_REF_TROUBLESHOOTING,
    },
    SkillRef {
        rel_path: "references/shell-and-overlay.md",
        body: SKILL_WINDOWS_REF_SHELL,
    },
];

const SKILL_LINUX_REFS: &[SkillRef] = &[
    SkillRef {
        rel_path: "references/capabilities.md",
        body: SKILL_LINUX_REF_CAPABILITIES,
    },
    SkillRef {
        rel_path: "references/bus-and-permissions.md",
        body: SKILL_LINUX_REF_BUS,
    },
    SkillRef {
        rel_path: "references/display-servers-and-input.md",
        body: SKILL_LINUX_REF_DISPLAY,
    },
    SkillRef {
        rel_path: "references/windows-and-lifecycle.md",
        body: SKILL_LINUX_REF_WINDOWS,
    },
    SkillRef {
        rel_path: "references/capture-and-clipboard.md",
        body: SKILL_LINUX_REF_CAPTURE,
    },
    SkillRef {
        rel_path: "references/shell-tray-and-notifications.md",
        body: SKILL_LINUX_REF_SHELL,
    },
    SkillRef {
        rel_path: "references/troubleshooting.md",
        body: SKILL_LINUX_REF_TROUBLESHOOTING,
    },
];

const SKILL_WEB_REFS: &[SkillRef] = &[SkillRef {
    rel_path: "references/reference.md",
    body: SKILL_WEB_REF_REFERENCE,
}];

pub(super) const SKILLS: &[Skill] = &[
    Skill {
        canonical: "agent-desktop",
        aliases: &["desktop", "agent-desktop"],
        summary: "Core guide for every OS: the snapshot and ref loop, when to read again, the retry rule, and which topic holds the detail. Load this first, then `skills get platform`.",
        platform: None,
        main: SKILL_DESKTOP_MAIN,
        refs: SKILL_DESKTOP_REFS,
    },
    Skill {
        canonical: "agent-desktop-macos",
        aliases: &["macos", "agent-desktop-macos"],
        summary: "macOS platform guide: Accessibility and Screen Recording permissions, what differs on macOS, Notification Center, troubleshooting.",
        platform: Some("macos"),
        main: SKILL_MACOS_MAIN,
        refs: SKILL_MACOS_REFS,
    },
    Skill {
        canonical: "agent-desktop-windows",
        aliases: &["windows", "agent-desktop-windows"],
        summary: "Windows platform guide: capability table, PowerShell quoting, supported sessions, shell surfaces and Action Center, UIPI/elevation, Chromium/Electron, troubleshooting.",
        platform: Some("windows"),
        main: SKILL_WINDOWS_MAIN,
        refs: SKILL_WINDOWS_REFS,
    },
    Skill {
        canonical: "agent-desktop-linux",
        aliases: &["linux", "agent-desktop-linux"],
        summary: "Linux platform guide: AT-SPI bus and permissions, X11 versus Wayland, window and app lifecycle, screenshot and clipboard, shell tray and notifications, troubleshooting.",
        platform: Some("linux"),
        main: SKILL_LINUX_MAIN,
        refs: SKILL_LINUX_REFS,
    },
    Skill {
        canonical: "agent-desktop-web",
        aliases: &["web", "agent-desktop-web"],
        summary: "Web pages in Chrome on their own: a short quickstart for the open, snapshot, find, act loop, with launch options, date and time inputs, long pages and every web error in its reference. Enough for web work without the desktop and platform skills.",
        platform: None,
        main: SKILL_WEB_MAIN,
        refs: SKILL_WEB_REFS,
    },
    Skill {
        canonical: "agent-desktop-ffi",
        aliases: &["ffi", "agent-desktop-ffi"],
        summary: "Embedding agent-desktop in another process via the C ABI. Build/link, error propagation, handle ownership, threading rules.",
        platform: None,
        main: SKILL_FFI_MAIN,
        refs: SKILL_FFI_REFS,
    },
    Skill {
        canonical: "jev-desktop",
        aliases: &["jev", "jev-desktop"],
        summary: "Driving a desktop app from a plain-language goal without reading the accessibility tree. run.mjs takes a whole goal, act.mjs takes one step.",
        platform: None,
        main: SKILL_JEV_MAIN,
        refs: &[],
    },
];
