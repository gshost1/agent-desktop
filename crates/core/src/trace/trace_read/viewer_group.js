const TraceViewerGroup = (() => {
  const writerKey = (event, seq) => JSON.stringify([event.segment || "", event.writer_pid, seq]);

  function claimedStarts(events) {
    const claimed = new Set();
    events.forEach((event) => {
      if (event.event === "command.end" && typeof event.command_seq === "number") {
        claimed.add(writerKey(event, event.command_seq));
      }
    });
    return claimed;
  }

  function closesFrame(frame, end) {
    const start = frame.start;
    if ((start.segment || "") !== (end.segment || "") || start.writer_pid !== end.writer_pid) return false;
    if (typeof end.command_seq === "number") return frame.claimed && start.seq === end.command_seq;
    return !frame.claimed && frame.command === (end.command || "command");
  }

  function groupEvents(events) {
    const claimed = claimedStarts(events);
    const groups = [];
    const open = [];
    events.forEach((event, index) => {
      const name = event.event || "";
      if (name === "command.start") {
        const frame = {
          command: event.command || "command",
          start: event,
          children: [],
          end: null,
          open: true,
          claimed: typeof event.seq === "number" && claimed.has(writerKey(event, event.seq)),
        };
        groups.push({type: "group", group: frame});
        open.push(frame);
        return;
      }
      if (name === "command.end") {
        const at = open.findIndex((frame) => closesFrame(frame, event));
        if (at >= 0) {
          const closed = open.splice(at, 1)[0];
          closed.end = event;
          closed.open = false;
        } else {
          groups.push({type: "event", event: event, index: index});
        }
        return;
      }
      if (open.length) open[open.length - 1].children.push({event: event, index: index});
      else groups.push({type: "event", event: event, index: index});
    });
    return groups;
  }

  return {closesFrame, groupEvents};
})();
if (typeof module !== "undefined") module.exports = TraceViewerGroup;
