use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use agent_desktop_core::{AdapterError, Deadline};
use futures_util::StreamExt;
use zbus::Message;
use zbus::connection::Builder;
use zbus::zvariant::{OwnedObjectPath, Value};

use crate::system::REGISTRY_ROOT_PATH;
use crate::tree::properties::liveness::Liveness;
use crate::tree::properties::node_read::read_node_with;
use crate::tree::properties::property_read::PropertyRead;
use crate::tree::properties::read_purpose::ReadPurpose;
use crate::tree::walk::walker_expand::LevelReader;
use crate::tree::{AtspiElement, RawNode, WalkLimits};

const UNKNOWN_PROPERTY: &str = "org.freedesktop.DBus.Error.UnknownProperty";
const UNKNOWN_INTERFACE: &str = "org.freedesktop.DBus.Error.UnknownInterface";
const UNKNOWN_OBJECT: &str = "org.freedesktop.DBus.Error.UnknownObject";
const SERVICE_UNKNOWN: &str = "org.freedesktop.DBus.Error.ServiceUnknown";
const ATTRIBUTE_ERROR: &str = "org.freedesktop.DBus.Error.Failed";

type Answer<T> = Result<T, &'static str>;
type ReadOutcome = Result<(RawNode, Liveness), (String, String)>;

/// How the scripted node answers `GetAll(Accessible)`.
#[derive(Clone, Copy)]
enum Group {
    Answers,
    Refuses(&'static str),
    Omits(&'static str),
    Mistypes(&'static str),
}

/// One node's answer to every call a node read sends.
#[derive(Clone)]
struct Script {
    role: Answer<u32>,
    name: Answer<&'static str>,
    description: Answer<&'static str>,
    child_count: Answer<i32>,
    accessible_id: Answer<&'static str>,
    attributes: Answer<&'static [(&'static str, &'static str)]>,
    group: (Group, bool),
}

fn live() -> Script {
    Script {
        role: Ok(43),
        name: Ok("Save"),
        description: Ok("Save document"),
        child_count: Ok(0),
        accessible_id: Ok("save"),
        attributes: Ok(&[("toolkit", "gtk"), ("id", "save")]),
        group: (Group::Answers, true),
    }
}

fn gone(error: &'static str) -> Script {
    Script {
        role: Err(error),
        name: Err(error),
        description: Err(error),
        child_count: Err(error),
        accessible_id: Err(error),
        attributes: Err(error),
        group: (Group::Refuses(error), false),
    }
}

fn property(script: &Script, key: &str) -> Answer<Value<'static>> {
    match key {
        "Name" => script.name.map(Value::from),
        "Description" => script.description.map(Value::from),
        "ChildCount" => script.child_count.map(Value::from),
        "AccessibleId" => script.accessible_id.map(Value::from),
        _ => Err(UNKNOWN_PROPERTY),
    }
}

fn attribute_map(script: &Script) -> Answer<HashMap<String, String>> {
    script.attributes.map(|pairs| {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    })
}

fn group(script: &Script) -> Answer<HashMap<String, Value<'static>>> {
    let (mode, carries) = script.group;
    if let Group::Refuses(error) = mode {
        return Err(error);
    }
    let mut all = HashMap::new();
    for key in ["Name", "Description", "ChildCount", "AccessibleId"] {
        let skipped = matches!(mode, Group::Omits(omitted) if omitted == key);
        match property(script, key) {
            Ok(_) if matches!(mode, Group::Mistypes(mistyped) if mistyped == key) => {
                all.insert(key.to_owned(), Value::from(7u8));
            }
            Ok(value) if !skipped => {
                all.insert(key.to_owned(), value);
            }
            _ => {}
        }
    }
    if let (true, Ok(map)) = (carries, attribute_map(script)) {
        all.insert("Attributes".to_owned(), Value::from(map));
    }
    all.insert("Locale".to_owned(), Value::from("en_US"));
    Ok(all)
}

async fn answer<T>(connection: &zbus::Connection, call: &Message, reply: Answer<T>)
where
    T: zbus::export::serde::Serialize + zbus::zvariant::DynamicType,
{
    let header = call.header();
    let sent = match reply {
        Ok(body) => connection.reply(&header, &body).await,
        Err(error) => connection.reply_error(&header, error, &("scripted",)).await,
    };
    sent.unwrap();
}

async fn respond(connection: &zbus::Connection, call: &Message, script: &Script, toolkit: &str) {
    let header = call.header();
    let path = header
        .path()
        .map(|path| path.to_string())
        .unwrap_or_default();
    let member = header
        .member()
        .map(|member| member.to_string())
        .unwrap_or_default();
    let interface = header
        .interface()
        .map(|name| name.to_string())
        .unwrap_or_default();
    if path == REGISTRY_ROOT_PATH {
        return match member.as_str() {
            "GetInterfaces" => {
                answer(connection, call, Ok(vec!["org.a11y.atspi.Application"])).await
            }
            _ => {
                let (_, key): (String, String) = call.body().deserialize().unwrap();
                let value = if key == "ToolkitName" { toolkit } else { "1.0" };
                answer(connection, call, Ok(Value::from(value))).await
            }
        };
    }
    match (interface.as_str(), member.as_str()) {
        (_, "Get") => {
            let (_, key): (String, String) = call.body().deserialize().unwrap();
            answer(connection, call, property(script, &key)).await
        }
        (_, "GetAll") => answer(connection, call, group(script)).await,
        (_, "GetRole") => answer(connection, call, script.role).await,
        (_, "GetAttributes") => answer(connection, call, attribute_map(script)).await,
        (_, "GetRoleName") => answer(connection, call, script.role.map(|_| "push button")).await,
        (_, "GetState") => answer(connection, call, script.role.map(|_| vec![0u32, 0])).await,
        (_, "GetInterfaces") => {
            answer(
                connection,
                call,
                script.role.map(|_| vec!["org.a11y.atspi.Accessible"]),
            )
            .await
        }
        (_, "GetChildren") => {
            answer(
                connection,
                call,
                script.role.map(|_| Vec::<(String, OwnedObjectPath)>::new()),
            )
            .await
        }
        _ => answer::<()>(connection, call, Err(UNKNOWN_INTERFACE)).await,
    }
}

/// A peer connection whose far end answers from `script` and records every
/// method it was sent as `Interface.Member`.
fn scripted(script: Script, toolkit: &'static str) -> (zbus::Connection, Arc<Mutex<Vec<String>>>) {
    let (near, far) = std::os::unix::net::UnixStream::pair().unwrap();
    let (client, server) = zbus::block_on(futures_lite::future::zip(
        Builder::async_io_unix_stream(near).p2p().build(),
        Builder::async_io_unix_stream(far)
            .server(zbus::Guid::generate())
            .unwrap()
            .p2p()
            .build(),
    ));
    let (client, server) = (client.unwrap(), server.unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let mut stream = zbus::MessageStream::from(&server);
    std::thread::spawn(move || {
        zbus::block_on(async {
            while let Some(Ok(call)) = stream.next().await {
                let header = call.header();
                if header.message_type() != zbus::message::Type::MethodCall {
                    continue;
                }
                let interface = header.interface().map(|name| name.to_string());
                let member = header.member().map(|member| member.to_string());
                let key = match member.as_deref() {
                    Some("Get") => call
                        .body()
                        .deserialize::<(String, String)>()
                        .map(|(_, key)| format!("({key})"))
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                log.lock().unwrap().push(format!(
                    "{}.{}{key}",
                    interface.unwrap_or_default(),
                    member.unwrap_or_default()
                ));
                respond(&server, &call, &script, toolkit).await;
            }
        });
    });
    (client, seen)
}

fn read(script: &Script, purpose: ReadPurpose, style: PropertyRead) -> (ReadOutcome, Vec<String>) {
    let (connection, seen) = scripted(script.clone(), "gtk");
    let element = AtspiElement::new(":1.1", "/node", 1);
    let outcome = zbus::block_on(read_node_with(&connection, &element, purpose, style))
        .map_err(|error: AdapterError| (format!("{:?}", error.code), error.message));
    let calls = seen.lock().unwrap().clone();
    (outcome, calls)
}

fn scripts() -> Vec<(&'static str, Script)> {
    let mut cases = vec![("every field present", live())];
    let mut failed_id = live();
    failed_id.accessible_id = Err(UNKNOWN_PROPERTY);
    cases.push(("one property failed, left out of the group", failed_id));
    let mut qt_like = live();
    qt_like.group = (Group::Refuses(UNKNOWN_INTERFACE), false);
    cases.push(("the group refused, the gets answer", qt_like));
    let mut omitted = live();
    omitted.group = (Group::Omits("Description"), true);
    cases.push(("the group leaves out a key the get answers", omitted));
    let mut mistyped = live();
    mistyped.group = (Group::Mistypes("ChildCount"), true);
    cases.push(("the group sends a key the get types differently", mistyped));
    let mut failed_attributes = live();
    failed_attributes.attributes = Err(ATTRIBUTE_ERROR);
    cases.push(("attributes failed, none carried", failed_attributes));
    let mut uncarried = live();
    uncarried.group = (Group::Answers, false);
    cases.push(("attributes not carried by the group", uncarried));
    let mut role_failed = live();
    role_failed.role = Err(ATTRIBUTE_ERROR);
    cases.push(("role failed on a node that answers the rest", role_failed));
    cases.push(("object gone", gone(UNKNOWN_OBJECT)));
    cases.push(("application gone", gone(SERVICE_UNKNOWN)));
    cases
}

const PURPOSES: [ReadPurpose; 4] = [
    ReadPurpose::Snapshot,
    ReadPurpose::Count,
    ReadPurpose::Actions,
    ReadPurpose::State,
];

#[test]
fn a_grouped_read_gives_the_evidence_of_the_separate_reads_in_every_field_state() {
    for (case, script) in scripts() {
        for purpose in PURPOSES {
            let (separate, _) = read(&script, purpose, PropertyRead::Separate);
            for style in [PropertyRead::Grouped, PropertyRead::GroupedWithAttributes] {
                let (grouped, _) = read(&script, purpose, style);
                assert_eq!(grouped, separate, "{case}, {purpose:?}, {style:?}");
            }
        }
    }
}

#[test]
fn the_cases_cover_present_absent_and_failed_fields_and_both_gone_answers() {
    let (present, _) = read(&live(), ReadPurpose::Snapshot, PropertyRead::Separate);
    let (node, _) = present.unwrap();
    assert!(node.properties.accessible_id.present().is_some());
    assert!(matches!(node.extents, crate::tree::Field::Absent));
    let mut failed_id = live();
    failed_id.accessible_id = Err(UNKNOWN_PROPERTY);
    let (failed, _) = read(&failed_id, ReadPurpose::Snapshot, PropertyRead::Separate);
    assert!(failed.unwrap().0.properties.accessible_id.is_failed());
    for (error, code) in [
        (UNKNOWN_OBJECT, "ElementNotFound"),
        (SERVICE_UNKNOWN, "AppNotFound"),
    ] {
        let (outcome, _) = read(&gone(error), ReadPurpose::Count, PropertyRead::Separate);
        assert_eq!(outcome.unwrap_err().0, code, "{error}");
    }
}

#[test]
fn a_group_that_answers_replaces_the_four_gets_and_the_carried_attributes_read() {
    let (_, grouped) = read(
        &live(),
        ReadPurpose::Count,
        PropertyRead::GroupedWithAttributes,
    );
    assert_eq!(
        grouped
            .iter()
            .filter(|call| call.ends_with("GetAll"))
            .count(),
        1
    );
    assert!(
        !grouped.iter().any(|call| call.contains(".Get(")),
        "{grouped:?}"
    );
    assert!(
        !grouped.iter().any(|call| call.ends_with("GetAttributes")),
        "{grouped:?}"
    );
    let (_, separate) = read(&live(), ReadPurpose::Count, PropertyRead::Separate);
    assert_eq!(
        separate.len(),
        grouped.len() + 4,
        "{separate:?} {grouped:?}"
    );
    let (_, uncarried) = read(&live(), ReadPurpose::Count, PropertyRead::Grouped);
    assert_eq!(uncarried.len(), grouped.len() + 1, "{uncarried:?}");
}

fn walk_calls(toolkit: &'static str) -> Vec<String> {
    let (connection, seen) = scripted(live(), toolkit);
    let root = AtspiElement::new(":1.1", "/node", 1);
    let limits = WalkLimits::default();
    let mut reader = LevelReader::new(&connection, &root, &limits, Deadline::standard().unwrap())
        .for_purpose(ReadPurpose::Count);
    reader.expand(&[root]).unwrap();
    seen.lock().unwrap().clone()
}

#[test]
fn a_walk_sends_the_group_read_only_to_a_toolkit_measured_to_answer_it() {
    for toolkit in ["Qt", "Gecko", "unknown"] {
        let calls = walk_calls(toolkit);
        assert!(
            !calls.iter().any(|call| call.ends_with("GetAll")),
            "{toolkit}: {calls:?}"
        );
        assert!(
            calls
                .iter()
                .any(|call| call.ends_with(".Get(AccessibleId)")),
            "{toolkit}"
        );
    }
    for toolkit in ["Chromium", "gtk"] {
        let calls = walk_calls(toolkit);
        assert!(
            calls.iter().any(|call| call.ends_with("GetAll")),
            "{toolkit}: {calls:?}"
        );
        assert!(
            !calls
                .iter()
                .any(|call| call.ends_with(".Get(AccessibleId)")),
            "{toolkit}"
        );
    }
}
