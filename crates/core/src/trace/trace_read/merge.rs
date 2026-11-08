use super::segment::ParsedEvent;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

#[derive(Debug, Clone)]
pub(crate) struct MergeItem {
    pub event: ParsedEvent,
    pub writer_pid: u32,
    pub segment: String,
}

struct StreamState {
    events: Vec<ParsedEvent>,
    index: usize,
    writer_pid: u32,
    segment: String,
}

type HeapKey = (u64, u32, u64, usize);

fn stream_head_key(streams: &[StreamState], stream_idx: usize) -> Option<HeapKey> {
    let stream = &streams[stream_idx];
    let event = stream.events.get(stream.index)?;
    Some((event.ts_ms, stream.writer_pid, event.position, stream_idx))
}

pub(crate) fn merge_segments(sources: Vec<(Vec<ParsedEvent>, u32, String)>) -> Vec<MergeItem> {
    let mut streams: Vec<StreamState> = sources
        .into_iter()
        .map(|(events, writer_pid, segment)| StreamState {
            events,
            index: 0,
            writer_pid,
            segment,
        })
        .collect();

    let mut heap: BinaryHeap<Reverse<HeapKey>> = (0..streams.len())
        .filter_map(|stream_idx| stream_head_key(&streams, stream_idx).map(Reverse))
        .collect();

    let mut merged = Vec::new();
    while let Some(Reverse((_, _, _, stream_idx))) = heap.pop() {
        let cursor = streams[stream_idx].index;
        merged.push(MergeItem {
            event: streams[stream_idx].events[cursor].clone(),
            writer_pid: streams[stream_idx].writer_pid,
            segment: streams[stream_idx].segment.clone(),
        });
        streams[stream_idx].index += 1;

        if let Some(key) = stream_head_key(&streams, stream_idx) {
            heap.push(Reverse(key));
        }
    }
    merged
}

use super::open_commands::{OpenCommands, command, event_name, segment, writer_pid};
use serde_json::{Value, json};

pub(crate) fn annotate_provenance(item: &MergeItem) -> Value {
    let mut value = item.event.value.clone();
    if let Value::Object(ref mut map) = value {
        map.entry("writer_pid".to_string())
            .or_insert(json!(item.writer_pid));
        map.entry("segment".to_string())
            .or_insert(json!(item.segment));
    }
    value
}

pub(crate) fn filter_by_event_prefix(events: Vec<Value>, prefix: Option<&str>) -> Vec<Value> {
    let Some(prefix) = prefix.filter(|p| !p.is_empty()) else {
        return events;
    };
    events
        .into_iter()
        .filter(|event| {
            event
                .get("event")
                .and_then(Value::as_str)
                .is_some_and(|name| name.starts_with(prefix))
        })
        .collect()
}

pub(crate) fn apply_tail_limit(mut events: Vec<Value>, limit: usize) -> (Vec<Value>, bool) {
    if limit == 0 || events.len() <= limit {
        return (events, false);
    }
    let start = events.len() - limit;
    (events.split_off(start), true)
}

pub(crate) fn detect_unpaired_commands(
    events: &[Value],
    current_command: Option<(u32, u64, &str)>,
) -> Vec<String> {
    let mut open = OpenCommands::new(events);
    let mut warnings = Vec::new();

    for (index, event) in events.iter().enumerate() {
        match event_name(event) {
            Some("command.start") => open.start(index, event),
            Some("command.end") if !open.end(event) => warnings.push(format!(
                "Unpaired command.end for '{}' in segment {}",
                command(event),
                segment(event)
            )),
            _ => {}
        }
    }

    for event in open
        .into_unpaired()
        .into_iter()
        .filter_map(|index| events.get(index))
    {
        if is_current_command(event, current_command) {
            continue;
        }
        let seq = event
            .get("seq")
            .and_then(Value::as_u64)
            .map(|seq| format!(" (seq {seq})"))
            .unwrap_or_default();
        warnings.push(format!(
            "Unpaired command.start for '{}' in segment {}{seq}",
            command(event),
            segment(event)
        ));
    }
    warnings
}

fn is_current_command(event: &Value, current_command: Option<(u32, u64, &str)>) -> bool {
    current_command.is_some_and(|(pid, proc_start_ms, current)| {
        command(event) == current
            && writer_pid(event) == u64::from(pid)
            && event.get("writer_proc_start_ms").and_then(Value::as_u64) == Some(proc_start_ms)
    })
}
