//! Runs one tool call as a child process of this same binary and maps its JSON envelope to an MCP result.

use serde_json::{Map, Value, json};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

use super::Server;
use super::catalog::Tool;
use super::child_lease::ChildLease;
use super::lease_inheritance::needs_lease;
use super::lease_slot::Ticket;

const CANCELLED: &str = "Cancelled";

/// Runs `tool` with `arguments` for request `id`, prefixed by the server's own global flags, and returns a
/// `CallToolResult`. A cancelled call returns early; the caller sends no response for it. A child lent the
/// server's inherited lease keeps it until the child has been reaped, here or by the cancel's reaper.
pub(super) fn call(
    server: &Server,
    tool: &Tool,
    arguments: &Map<String, Value>,
    id: &Value,
    ticket: Option<Ticket>,
) -> Value {
    let argv = match tool.argv(arguments) {
        Ok(argv) => argv,
        Err(message) => return error_result(&message),
    };
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => return error_result(&format!("Cannot locate agent-desktop: {error}")),
    };
    let Some(lease) = server
        .lease
        .child_lease(ticket, || needs_lease(&server.globals, &argv))
    else {
        return error_result(CANCELLED);
    };
    let mut command = child_command(&exe, &server.globals, &argv, &lease);
    let (stdout, stderr) = match server.calls.start(id, &mut command) {
        None => return error_result(CANCELLED),
        Some(Err(error)) => return error_result(&format!("Cannot run agent-desktop: {error}")),
        Some(Ok(pipes)) => pipes,
    };
    let (stdout, stderr) = std::thread::scope(|scope| {
        let stderr = scope.spawn(|| read_all(stderr));
        (read_all(stdout), stderr.join().unwrap_or_default())
    });
    let Some(mut child) = server.calls.take(id) else {
        return error_result(CANCELLED);
    };
    let status = child
        .wait()
        .map_or_else(|error| error.to_string(), |status| status.to_string());
    match serde_json::from_slice::<Value>(&stdout) {
        Ok(envelope) => envelope_result(envelope),
        Err(_) => error_result(&format!(
            "agent-desktop {} exited with {status} and no JSON envelope: {}",
            tool.path.join(" "),
            String::from_utf8_lossy(&stderr).trim()
        )),
    }
}

/// The child process for one call: this binary with the server's global flags, `argv`, and what `lease` gives it.
pub(crate) fn child_command(
    exe: &Path,
    globals: &[String],
    argv: &[String],
    lease: &ChildLease<'_>,
) -> Command {
    let mut command = Command::new(exe);
    command
        .args(globals)
        .args(argv)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    lease.apply(&mut command);
    command
}

fn read_all(pipe: Option<impl Read>) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_end(&mut bytes);
    }
    bytes
}

/// Maps an envelope to a result: the envelope as structured content and text, an image block for inline screenshots.
pub(crate) fn envelope_result(mut envelope: Value) -> Value {
    let is_error = envelope["ok"] != Value::Bool(true);
    let image = take_inline_image(&mut envelope);
    let mut content = vec![json!({ "type": "text", "text": envelope.to_string() })];
    content.extend(image);
    json!({ "content": content, "structuredContent": envelope, "isError": is_error })
}

fn take_inline_image(envelope: &mut Value) -> Option<Value> {
    let data = envelope.get_mut("data")?.as_object_mut()?;
    let mime_type = match data.get("format")?.as_str()? {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => return None,
    };
    if !data.get("data").is_some_and(Value::is_string) {
        return None;
    }
    let Value::String(encoded) = data.remove("data")? else {
        return None;
    };
    Some(json!({ "type": "image", "data": encoded, "mimeType": mime_type }))
}

fn error_result(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}
