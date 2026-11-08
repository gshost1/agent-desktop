#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use agent_desktop_core::{AccessibilityNode, Deadline};
use agent_desktop_web::{
    CdpConnection, ElementToken, PageSession, WebSnapshot, attach_to_target, connect,
    create_background_tab, endpoint_from_user_data_dir, navigate,
};

const CHROME: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

pub struct HeadlessChrome {
    child: Child,
    pub profile: PathBuf,
}

impl Drop for HeadlessChrome {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let flag = format!("--user-data-dir={}", self.profile.display());
        let started = Instant::now();
        while helpers_running(&flag) {
            if started.elapsed() > Duration::from_secs(10) {
                return;
            }
            let _ = Command::new("pkill")
                .args(["-KILL", "-f", "--", &flag])
                .status();
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}

fn helpers_running(flag: &str) -> bool {
    Command::new("pgrep")
        .args(["-f", "--", flag])
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Starts headless Chrome with a throwaway profile under the target
/// directory and every site in its own process, so a cross-site iframe is
/// always an out-of-process frame.
pub fn launch(name: &str) -> HeadlessChrome {
    let profile = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("web-live-{name}-{}", std::process::id()));
    assert!(
        !profile.exists(),
        "a live test profile folder is reused: {}",
        profile.display()
    );
    std::fs::create_dir_all(&profile).unwrap();
    let child = Command::new(CHROME)
        .arg("--headless=new")
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg("--remote-debugging-port=0")
        .arg("--site-per-process")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("about:blank")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let chrome = HeadlessChrome { child, profile };
    let started = Instant::now();
    while endpoint_from_user_data_dir(&chrome.profile).is_err() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "Chrome never wrote a complete DevToolsActivePort"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    chrome
}

pub fn deadline() -> Deadline {
    Deadline::after(10_000).unwrap()
}

pub fn connect_to(chrome: &HeadlessChrome) -> CdpConnection {
    let endpoint = endpoint_from_user_data_dir(&chrome.profile).unwrap();
    connect(&endpoint, deadline()).unwrap()
}

pub fn open_tab(connection: &mut CdpConnection, url: &str) -> PageSession {
    let target_id = create_background_tab(connection, "about:blank", deadline()).unwrap();
    let session = attach_to_target(connection, &target_id, deadline()).unwrap();
    navigate(connection, &session, url, deadline()).unwrap();
    session
}

pub fn data_url(html: &str) -> String {
    let mut url = String::from("data:text/html;charset=utf-8,");
    for byte in html.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            url.push(byte as char);
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    url
}

/// Serves `html` for every request on a loopback port and returns the port.
/// Each connection gets its own thread, so a preconnected socket that never
/// sends a request cannot hold up the page load behind it.
pub fn serve(html: String) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let html = std::sync::Arc::new(html);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let html = std::sync::Arc::clone(&html);
            std::thread::spawn(move || answer(stream, &html));
        }
    });
    port
}

fn answer(mut stream: std::net::TcpStream, html: &str) {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 1024];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(read) => request.extend_from_slice(&chunk[..read]),
        }
    }
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

pub fn find<'a>(
    snapshot: &'a WebSnapshot,
    role: &str,
    name: &str,
) -> (&'a AccessibilityNode, ElementToken) {
    let (node, origin) = snapshot
        .preorder()
        .into_iter()
        .find(|(node, _)| node.role == role && node.identity.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {role} named {name}: {:#?}", snapshot.tree));
    let token = origin.unwrap().element_token();
    if let Some(retained) = node.identity.retained_object.as_deref() {
        assert_eq!(ElementToken::parse(retained).unwrap(), token);
    }
    (node, token)
}
