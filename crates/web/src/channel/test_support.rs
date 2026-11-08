use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use agent_desktop_core::system::cdp_endpoint::CdpEndpoint;
use agent_desktop_core::{
    AccessibilityNode, ActionOps, ActionRequest, ActionResult, AdapterError, Deadline, InputOps,
    InteractionLease, LiveElement, LocatorStats, NativeHandle, NodeIdentity, NodePresentation,
    ObservationOps, ObservationRequest, ObservationRoot, ObservationSource, ObservedSubtree,
    ObservedTree, OpenUrlRequest, OpenedPage, ProcessId, ProcessIdentity, RefCapabilities,
    RefEntry, RefEntryIdentity, RefGeometry, RefProcess, RefScope, RefSource, SnapshotSurface,
    SurfaceInfo, SystemOps,
};
use serde_json::{Value, json};

use super::candidate_origin::CandidateOrigin;
use super::cdp_connector::CdpConnector;
use super::endpoint_candidate::EndpointCandidate;
use super::observed_page::node_evidence;
use super::web_channel_adapter::WebChannelAdapter;
use crate::CdpConnection;
use crate::transport::scripted_transport::ScriptedTransport;

pub(crate) const PID: u32 = 41;
pub(crate) const INSTANCE: &str = "inst-41";
pub(crate) const PORT: u16 = 9333;

pub(crate) fn process() -> ProcessIdentity {
    ProcessIdentity::new(PID, INSTANCE)
}

pub(crate) fn deadline() -> Deadline {
    Deadline::after(2_000).unwrap()
}

pub(crate) fn lease() -> InteractionLease {
    InteractionLease::guarded(deadline(), ()).unwrap()
}

/// Offers scripted endpoints and opens each with the next scripted transport.
#[derive(Default)]
pub(crate) struct ScriptedConnector {
    pub ports: Vec<u16>,
    pub connections: Mutex<VecDeque<ScriptedTransport>>,
    pub opens: Arc<Mutex<Vec<(u16, bool)>>>,
    pub open_error: Option<fn() -> AdapterError>,
    pub remembered: Arc<Mutex<Vec<(ProcessIdentity, u16)>>>,
    pub download_dir: Option<std::path::PathBuf>,
}

impl ScriptedConnector {
    pub(crate) fn with(transport: ScriptedTransport) -> Self {
        Self {
            ports: vec![PORT],
            connections: Mutex::new(VecDeque::from([transport])),
            ..Self::default()
        }
    }
}

impl CdpConnector for ScriptedConnector {
    fn candidates(&self, _process: &ProcessIdentity) -> Vec<EndpointCandidate> {
        self.ports
            .iter()
            .map(|port| EndpointCandidate {
                port: *port,
                origin: CandidateOrigin::Registry {
                    websocket_url: format!("ws://127.0.0.1:{port}/devtools/browser/x"),
                    download_dir: self.download_dir.clone(),
                },
            })
            .collect()
    }

    fn open(
        &self,
        candidate: &EndpointCandidate,
        _deadline: Deadline,
        may_prompt: bool,
    ) -> Result<CdpConnection, AdapterError> {
        self.opens
            .lock()
            .unwrap()
            .push((candidate.port, may_prompt));
        if let Some(error) = self.open_error {
            return Err(error());
        }
        let transport = self.connections.lock().unwrap().pop_front();
        Ok(CdpConnection::new(transport.unwrap_or_default().boxed()))
    }

    fn remember(
        &self,
        process: &ProcessIdentity,
        endpoint: &CdpEndpoint,
        _deadline: Deadline,
    ) -> Result<(), AdapterError> {
        self.remembered
            .lock()
            .unwrap()
            .push((process.clone(), endpoint.port));
        Ok(())
    }
}

/// A platform adapter that records what reaches it. It owns the listener on
/// `owned_ports`, reads one native button for any window, and answers every
/// handle it is given as a native one.
#[derive(Default)]
pub(crate) struct StubPlatform {
    pub owned_ports: Vec<u16>,
    pub calls: Mutex<Vec<String>>,
    pub surfaces_seen: Mutex<Vec<SnapshotSurface>>,
}

impl StubPlatform {
    pub(crate) fn owning(port: u16) -> Self {
        Self {
            owned_ports: vec![port],
            ..Self::default()
        }
    }

    fn record(&self, call: &str) {
        self.calls.lock().unwrap().push(call.into());
    }

    pub(crate) fn recorded(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

pub(crate) fn node(role: &str, name: &str, token: Option<&str>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: NodeIdentity {
            retained_object: token.map(str::to_owned),
            name: Some(name.into()),
            ..NodeIdentity::default()
        },
        presentation: NodePresentation {
            available_actions: vec!["Click".into()],
            ..NodePresentation::default()
        },
        children_count: None,
        subtree_truncated: false,
        children: Vec::new(),
    }
}

impl ObservationOps for StubPlatform {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        self.record("observe_tree");
        self.surfaces_seen.lock().unwrap().push(request.surface);
        let button = node("button", "Native", Some("axtok1:native"));
        ObservedTree::from_roots(
            vec![ObservedSubtree::new(
                node_evidence(&button),
                Vec::new(),
                true,
                None,
            )],
            ObservationSource::from_root(&root, request.surface),
            LocatorStats::default(),
            true,
        )
    }

    fn resolve_element_strict(
        &self,
        entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.record("resolve_element_strict");
        self.surfaces_seen
            .lock()
            .unwrap()
            .push(entry.source.source_surface);
        Ok(NativeHandle::new("native"))
    }

    fn get_live_element(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<LiveElement, AdapterError> {
        self.record("get_live_element");
        Err(AdapterError::not_supported("get_live_element"))
    }

    fn list_surfaces(
        &self,
        _process: ProcessIdentity,
        _deadline: Deadline,
    ) -> Result<Vec<SurfaceInfo>, AdapterError> {
        Ok(vec![SurfaceInfo {
            id: "w1".into(),
            kind: "window".into(),
            title: Some("Browser".into()),
            item_count: None,
            unclassified: Vec::new(),
        }])
    }
}

impl ActionOps for StubPlatform {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        request: ActionRequest,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.record("execute_action");
        Ok(ActionResult::delivered_unverified(request.action.name()))
    }
}

impl InputOps for StubPlatform {}

impl SystemOps for StubPlatform {
    fn process_owns_tcp_listener(
        &self,
        _process: ProcessIdentity,
        port: u16,
        _deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        Ok(self.owned_ports.contains(&port))
    }

    fn open_url(&self, _request: &OpenUrlRequest<'_>) -> Result<OpenedPage, AdapterError> {
        self.record("open_url");
        Err(AdapterError::not_supported("open_url"))
    }

    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        vec![SnapshotSurface::Window]
    }
}

pub(crate) fn adapter(
    platform: StubPlatform,
    connector: ScriptedConnector,
) -> WebChannelAdapter<StubPlatform> {
    WebChannelAdapter::with_connector(platform, Box::new(connector)).with_download_ledger(None)
}

/// A ref captured from a page over CDP.
pub(crate) fn web_entry(token: &str, role: &str, name: &str) -> RefEntry {
    RefEntry {
        process: RefProcess {
            pid: ProcessId::new(PID),
            process_instance: Some(INSTANCE.into()),
        },
        identity: RefEntryIdentity {
            retained_object: Some(token.into()),
            role: role.into(),
            name: Some(name.into()),
            unique_at_capture: true,
            ..RefEntryIdentity::default()
        },
        geometry: RefGeometry {
            bounds_space: None,
            bounds: None,
            bounds_hash: None,
        },
        capabilities: RefCapabilities {
            states: Vec::new(),
            available_actions: vec!["Click".into()],
        },
        source: RefSource {
            source_surface: SnapshotSurface::Web,
            ..RefSource::default()
        },
        scope: RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: Default::default(),
        },
    }
}

pub(crate) fn role(name: &str) -> Value {
    json!({ "type": "role", "value": name })
}

pub(crate) fn text(value: &str) -> Value {
    json!({ "type": "computedString", "value": value })
}

/// Expects the attach of `target` as session `S-<target>`.
pub(crate) fn attach(transport: ScriptedTransport, target: &str) -> ScriptedTransport {
    transport
        .expect(
            "Target.attachToTarget",
            json!({ "sessionId": format!("S-{target}") }),
        )
        .expect("Runtime.runIfWaitingForDebugger", json!({}))
}

/// Node 7 of loader `L` in frame `F` of target `T`.
pub(crate) const TOKEN: &str = "cdp:T:F:L:7";

/// Expects the document proof that frame `F` still shows loader `L`.
pub(crate) fn proven(transport: ScriptedTransport) -> ScriptedTransport {
    let frame = json!({ "frameTree": { "frame": { "id": "F", "loaderId": "L" } } });
    transport.expect("Page.getFrameTree", frame)
}

/// Expects one `read_node` of a live node.
pub(crate) fn node_read(
    transport: ScriptedTransport,
    backend: i64,
    node_role: &str,
    name: &str,
) -> ScriptedTransport {
    proven(transport).expect(
            "Accessibility.getPartialAXTree",
            json!({ "nodes": [{
                "nodeId": "1", "backendDOMNodeId": backend, "ignored": false,
                "role": role(node_role), "name": text(name),
                "properties": [{ "name": "focusable", "value": { "type": "booleanOrUndefined", "value": true } }]
            }] }),
        )
}

/// Expects one `read_element` that reports `value` and `checked`.
pub(crate) fn element_read(
    transport: ScriptedTransport,
    value: Value,
    checked: Value,
) -> ScriptedTransport {
    proven(transport)
        .expect("DOM.resolveNode", json!({ "object": { "objectId": "r" } }))
        .expect(
            "Runtime.callFunctionOn",
            json!({ "result": { "value": { "value": value, "text": null, "checked": checked } } }),
        )
        .expect("Runtime.releaseObject", json!({}))
}

/// Expects `Target.getTargets` listing `(id, url, title)` pages.
pub(crate) fn targets(
    transport: ScriptedTransport,
    pages: &[(&str, &str, &str)],
) -> ScriptedTransport {
    let infos: Vec<Value> = pages
        .iter()
        .map(|(id, url, title)| json!({ "targetId": id, "type": "page", "url": url, "title": title }))
        .collect();
    transport.expect("Target.getTargets", json!({ "targetInfos": infos }))
}

/// Expects `Browser.getWindowForTarget` placing a target in window `id` at `left`.
pub(crate) fn window_of(transport: ScriptedTransport, id: i64, left: f64) -> ScriptedTransport {
    transport.expect(
        "Browser.getWindowForTarget",
        json!({ "windowId": id, "bounds": { "left": left, "top": 0, "width": 800, "height": 600 } }),
    )
}

/// Expects one isolated-world visibility read answering `state`.
pub(crate) fn visibility(transport: ScriptedTransport, state: &str) -> ScriptedTransport {
    transport
        .expect(
            "Page.getFrameTree",
            json!({ "frameTree": { "frame": { "id": "F" } } }),
        )
        .expect(
            "Page.createIsolatedWorld",
            json!({ "executionContextId": 5 }),
        )
        .expect(
            "Runtime.evaluate",
            json!({ "result": { "type": "string", "value": state } }),
        )
}

/// Expects a one-frame page snapshot holding a button and a textbox.
pub(crate) fn page_snapshot(transport: ScriptedTransport) -> ScriptedTransport {
    transport
        .expect(
            "Page.getFrameTree",
            json!({ "frameTree": { "frame": { "id": "MAIN", "loaderId": "LOAD" } } }),
        )
        .expect("DOMSnapshot.captureSnapshot", json!({}))
        .expect(
            "Accessibility.getFullAXTree",
            json!({ "nodes": [
                { "nodeId": "1", "role": role("RootWebArea"), "name": text("Docs"), "childIds": ["2", "3"], "backendDOMNodeId": 1 },
                { "nodeId": "2", "role": role("button"), "name": text("Go"), "backendDOMNodeId": 20 },
                { "nodeId": "3", "role": role("textbox"), "name": text("Query"), "backendDOMNodeId": 30 },
            ] }),
        )
        .expect("Target.setAutoAttach", json!({}))
        .expect("Target.setAutoAttach", json!({}))
}
