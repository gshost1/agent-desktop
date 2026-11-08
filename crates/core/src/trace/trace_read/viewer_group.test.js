const {test} = require("node:test");
const assert = require("node:assert/strict");
const {groupEvents} = require("./viewer_group.js");

const event = (name, seq, extra) =>
  Object.assign({event: name, seq: seq, command: "snapshot", segment: "100-1", writer_pid: 100}, extra || {});

const pairs = (events) =>
  groupEvents(events).map((item) =>
    item.type === "group" ? [item.group.start.seq, item.group.end ? item.group.end.seq : null] : ["end", item.event.seq]);

test("overlapping scopes of one command close by command_seq", () => {
  const events = [
    event("command.start", 1),
    event("command.start", 2),
    event("command.end", 3, {command_seq: 1}),
    event("command.end", 4, {command_seq: 2}),
  ];
  assert.deepEqual(pairs(events), [[1, 3], [2, 4]]);
});

test("a lost end leaves only its own scope open", () => {
  const events = [event("command.start", 1), event("command.start", 2), event("command.end", 3, {command_seq: 2})];
  assert.deepEqual(pairs(events), [[1, null], [2, 3]]);
});

test("a legacy end leaves alone a start that a later end claims by seq", () => {
  const mixed = [
    event("command.start", 1),
    event("command.start", 2),
    event("command.end", 3),
    event("command.end", 4, {command_seq: 1}),
  ];
  assert.deepEqual(pairs(mixed), [[1, 4], [2, 3]]);
  const unwrittenStart = [event("command.start", 1), event("command.end", 2), event("command.end", 3, {command_seq: 1})];
  assert.deepEqual(pairs(unwrittenStart), [[1, 3], ["end", 2]]);
});

test("legacy ends close the oldest open start of the same writer and command", () => {
  const events = [
    event("command.start", 1),
    event("command.start", 2, {command: "click"}),
    event("command.start", 3),
    event("command.start", 4, {segment: "200-1", writer_pid: 200}),
    event("command.end", 5, {command: "click"}),
    event("command.end", 6),
    event("command.end", 7, {command: "click"}),
  ];
  assert.deepEqual(pairs(events), [[1, 6], [2, 5], [3, null], [4, null], ["end", 7]]);
});

test("an end naming a start of another writer stays unpaired", () => {
  const events = [event("command.start", 1), event("command.end", 2, {command_seq: 1, writer_pid: 7})];
  assert.deepEqual(pairs(events), [[1, null], ["end", 2]]);
});
