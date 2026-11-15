use std::path::{Path, PathBuf};
use std::process::Command;

const BRIDGES: &[(&str, &str, bool, &str)] = &[
    (
        "src/system/cursor_overlay/cursor_overlay_display_bridge.m",
        "cursor_overlay_display_bridge.o",
        false,
        "compile Objective-C cursor overlay display bridge",
    ),
    (
        "src/system/cursor_overlay/cursor_overlay_bridge.m",
        "cursor_overlay_bridge.o",
        false,
        "compile Objective-C cursor overlay bridge",
    ),
    (
        "src/system/cursor_overlay/cursor_overlay_chrome_bridge.m",
        "cursor_overlay_chrome_bridge.o",
        false,
        "compile Objective-C cursor overlay chrome bridge",
    ),
    (
        "src/system/cursor_overlay/cursor_overlay_image_bridge.m",
        "cursor_overlay_image_bridge.o",
        false,
        "compile Objective-C cursor overlay image bridge",
    ),
    (
        "src/system/appkit_bridge.m",
        "appkit_bridge.o",
        true,
        "compile Objective-C AppKit bridge",
    ),
    (
        "src/system/launch_bridge.m",
        "launch_bridge.o",
        true,
        "compile Objective-C launch bridge",
    ),
    (
        "src/system/screen_bridge.m",
        "screen_bridge.o",
        false,
        "compile Objective-C screen bridge",
    ),
    (
        "src/system/permissions/permission_guide_bridge.m",
        "permission_guide_bridge.o",
        true,
        "compile Objective-C permission guide bridge",
    ),
];

fn main() {
    for (source, _, _, _) in BRIDGES {
        println!("cargo:rerun-if-changed={source}");
    }
    println!("cargo:rerun-if-changed=src/system/cursor_overlay/cursor_overlay_chrome.h");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        eprintln!("agent-desktop-macos only builds for macOS targets");
        std::process::exit(1);
    }
    let target = required_target();
    let deployment = deployment_target();
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "unknown".into());
    println!("cargo:rustc-env=AGENT_DESKTOP_MACOS_HELPER_BUILD_ID={version}:{target}:{deployment}");
    let Some(out_dir) = std::env::var_os("OUT_DIR").map(PathBuf::from) else {
        eprintln!("OUT_DIR is required");
        std::process::exit(1);
    };
    let objects: Vec<PathBuf> = BRIDGES
        .iter()
        .map(|spec| compile_bridge(&out_dir, &target, &deployment, *spec))
        .collect();
    let archive = out_dir.join("libagent_desktop_launch_bridge.a");
    run(
        Command::new("xcrun")
            .args(["--sdk", "macosx", "ar", "rcs"])
            .arg(&archive)
            .args(&objects),
        "archive Objective-C launch bridge",
    );
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=agent_desktop_launch_bridge");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=QuartzCore");
}

fn compile_bridge(
    out_dir: &Path,
    target: &str,
    deployment: &str,
    spec: (&str, &str, bool, &str),
) -> PathBuf {
    let (source, object_name, blocks, label) = spec;
    let object = out_dir.join(object_name);
    let mut command = Command::new("xcrun");
    command
        .args(["--sdk", "macosx", "clang", "-fobjc-arc"])
        .args(blocks.then_some("-fblocks"))
        .args(["-target", target])
        .arg(format!("-mmacosx-version-min={deployment}"))
        .args(["-Wall", "-Wextra", "-Werror", "-c", source, "-o"])
        .arg(&object);
    run(&mut command, label);
    object
}

fn required_target() -> String {
    match std::env::var("TARGET") {
        Ok(target)
            if matches!(
                target.as_str(),
                "aarch64-apple-darwin" | "x86_64-apple-darwin"
            ) =>
        {
            target
        }
        Ok(target) => {
            eprintln!("unsupported macOS launch bridge target: {target}");
            std::process::exit(1);
        }
        Err(error) => {
            eprintln!("TARGET is required: {error}");
            std::process::exit(1);
        }
    }
}

fn deployment_target() -> String {
    let deployment = std::env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "10.15".into());
    let supported = deployment
        .split_once('.')
        .and_then(|(major, minor)| Some((major.parse::<u32>().ok()?, minor.parse::<u32>().ok()?)))
        .is_some_and(|version| version >= (10, 15));
    if supported {
        deployment
    } else {
        eprintln!("MACOSX_DEPLOYMENT_TARGET must be 10.15 or newer");
        std::process::exit(1);
    }
}

fn run(command: &mut Command, label: &str) {
    match command.status() {
        Ok(status) if status.success() => {}
        Ok(status) => {
            eprintln!("{label} failed with {status}");
            std::process::exit(1);
        }
        Err(error) => {
            eprintln!("{label}: {error}");
            std::process::exit(1);
        }
    }
}
