/// How a browser endpoint was discovered. A `DevToolsActivePort` endpoint
/// may belong to a browser whose remote-debugging server asks its user to
/// approve each connection, so connecting to one waits for that approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointSource {
    JsonVersion,
    DevToolsActivePort,
}
