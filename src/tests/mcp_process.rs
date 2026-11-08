use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
}

fn serve(requests: &[Value], args: &[&str]) -> Vec<Value> {
    let mut child = binary()
        .arg("mcp")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("binary starts");
    let mut stdin = child.stdin.take().expect("stdin");
    for request in requests {
        writeln!(stdin, "{request}").expect("request written");
    }
    drop(stdin);
    let output = child
        .wait_with_output()
        .expect("server exits after stdin closes");
    assert!(output.status.success());
    let mut responses = String::from_utf8(output.stdout)
        .expect("utf-8")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("one JSON-RPC message per line"))
        .collect::<Vec<_>>();
    responses.sort_by_key(|response| response["id"].as_i64());
    responses
}

fn call(id: i64, name: &str, arguments: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": name, "arguments": arguments } })
}

#[test]
fn mcp_tool_calls_run_the_cli_command_and_return_its_envelope() {
    let responses = serve(
        &[
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                    "params": { "protocolVersion": "2025-11-25", "capabilities": {},
                                "clientInfo": { "name": "test", "version": "0" } } }),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            call(2, "desktop_version", json!({})),
            call(
                3,
                "desktop_skills_get",
                json!({ "name": "desktop", "reference": null }),
            ),
        ],
        &[],
    );
    assert_eq!(responses.len(), 3, "{responses:?}");
    assert_eq!(responses[0]["result"]["protocolVersion"], "2025-11-25");
    let version = &responses[1]["result"];
    assert_eq!(version["isError"], false, "{version}");
    assert_eq!(version["structuredContent"]["command"], "version");
    assert_eq!(
        version["structuredContent"]["data"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    let skill = &responses[2]["result"];
    assert_eq!(skill["isError"], false, "{skill}");
    assert!(
        skill["structuredContent"]["data"]["content"]
            .as_str()
            .is_some_and(|text| !text.is_empty())
    );
}

struct ScratchDir(std::path::PathBuf);

impl ScratchDir {
    fn new(label: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "agent-desktop-mcp-{label}-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn mcp_server_global_flags_reach_every_call() {
    let dir = ScratchDir::new("forward");
    let trace = dir.0.join("trace.jsonl");
    let trace_arg = trace.to_str().expect("utf-8 path");
    let responses = serve(
        &[call(1, "desktop_session_list", json!({}))],
        &["--trace", trace_arg, "--session", "mcp-forward-s1"],
    );
    let result = &responses[0]["result"];
    assert_eq!(result["isError"], false, "{result}");
    let written = std::fs::read_to_string(&trace).expect("the call wrote the server's trace");
    assert!(
        written.contains("\"command\":\"session\"")
            && written.contains("\"session_id\":\"mcp-forward-s1\""),
        "{written}"
    );
}

#[test]
fn mcp_refuses_per_command_globals_and_invalid_identity_before_reading_stdin() {
    for (args, named) in [
        (&["--debug", "--screenshot", "debug.html"][..], "--debug"),
        (&["--wait-for", "button:OK"], "--wait-for"),
        (&["--wait-timeout", "5"], "--wait-timeout"),
        (&["--agent-id", "not a valid id"], "Agent id"),
    ] {
        let mut child = binary()
            .arg("mcp")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("binary starts");
        let stdin = child.stdin.take().expect("stdin");
        let started = std::time::Instant::now();
        while child.try_wait().expect("child status").is_none() {
            if started.elapsed() > std::time::Duration::from_secs(20) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("{args:?}: mcp kept serving with stdin open instead of refusing");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        drop(stdin);
        let output = child.wait_with_output().expect("refusal output");
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(
            output.stdout.is_empty(),
            "{args:?}: stdout is JSON-RPC only"
        );
        let envelope: Value =
            serde_json::from_slice(&output.stderr).expect("one JSON envelope on stderr");
        assert_eq!(envelope["ok"], false, "{envelope}");
        assert_eq!(envelope["command"], "mcp", "{envelope}");
        assert_eq!(envelope["error"]["code"], "INVALID_ARGS", "{envelope}");
        assert!(envelope["error"]["suggestion"].is_string(), "{envelope}");
        assert!(
            envelope["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains(named)),
            "{envelope}"
        );
    }
}

#[test]
fn malformed_messages_get_errors_and_the_server_keeps_serving() {
    let responses = serve(
        &[
            json!([{ "jsonrpc": "2.0", "id": 9, "method": "tools/list" }]),
            json!(42),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "no/such/method" }),
            call(4, "desktop_skills_get", json!({ "name": "a\u{0}b" })),
            call(5, "desktop_version", json!({})),
        ],
        &[],
    );
    let codes = responses
        .iter()
        .map(|response| (response["id"].clone(), response["error"]["code"].clone()))
        .collect::<Vec<_>>();
    assert_eq!(codes[0], (Value::Null, json!(-32600)), "{responses:?}");
    assert_eq!(codes[1], (Value::Null, json!(-32600)), "{responses:?}");
    assert_eq!(codes[2], (json!(2), json!(-32601)), "{responses:?}");
    let nul = &responses[3]["result"];
    assert!(
        nul["content"][0]["text"]
            .as_str()
            .is_some_and(|text| text.starts_with("Cannot run agent-desktop")),
        "{nul}"
    );
    assert_eq!(responses[4]["result"]["isError"], false);
}
