//! Live headless-Chrome check that date, time, datetime-local, month, week,
//! color and range inputs take a whole value: the snapshot gives each a ref
//! that advertises `SetValue`, `fill` assigns the value in the input's own
//! format with bubbling `input` and `change` events and reads it back, and
//! one part of a time input, a malformed value and a rejected value all
//! fail without changing the input. The page is served from loopback.
//! Ignored by default; run with
//! `cargo test -p agent-desktop-web --test live_value_inputs -- --ignored`.

mod common;

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};
use agent_desktop_web::{
    CdpConnection, ElementToken, FillOutcome, PageSession, WebSnapshot, fill, snapshot_page,
    wait_for_load,
};
use common::{connect_to, deadline, find, launch, serve};
use serde_json::{Value, json};

const ORDER: &str = r#"<!doctype html><html><head><title>Pizza order</title></head><body>
<form id="order">
<label>Customer name <input id="customer" type="text"></label>
<label>Delivery time <input id="delivery" type="time" name="delivery"></label>
<label>Delivery day <input id="day" type="date" min="2026-01-01"></label>
<label>Pickup slot <input id="slot" type="datetime-local"></label>
<label>Billing month <input id="billing" type="month"></label>
<label>Roster week <input id="roster" type="week"></label>
<label>Box color <input id="box" type="color"></label>
<label>Spice level <input id="spice" type="range" min="0" max="10"></label>
<label>Tracked time <input id="tracked" type="time"></label>
</form>
<script>
window.events = [];
for (const input of document.querySelectorAll('input')) {
  for (const type of ['input', 'change']) {
    input.addEventListener(type, (event) => window.events.push(input.id + ':' + type + ':' + event.bubbles));
  }
}
document.getElementById('order').addEventListener('change', (event) => window.events.push('form:change:' + event.target.id));
const tracked = document.getElementById('tracked');
const native = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
let last = tracked.value;
Object.defineProperty(tracked, 'value', {
  configurable: true,
  get() { return native.get.call(this); },
  set(next) { last = String(next); native.set.call(this, next); },
});
tracked.addEventListener('input', () => {
  if (native.get.call(tracked) !== last) { last = native.get.call(tracked); window.events.push('tracked:noticed'); }
});
</script></body></html>"#;

struct Page {
    connection: CdpConnection,
    tab: PageSession,
    snapshot: WebSnapshot,
}

impl Page {
    fn set(&mut self, role: &str, name: &str, text: &str) -> Result<FillOutcome, AdapterError> {
        let (node, token) = find(&self.snapshot, role, name);
        assert!(
            node.presentation
                .available_actions
                .iter()
                .any(|action| action == "SetValue"),
            "{role} {name} does not advertise SetValue: {:?}",
            node.presentation.available_actions
        );
        assert!(
            node.identity.retained_object.is_some(),
            "{role} {name} has no ref"
        );
        self.fill(&token, text)
    }

    fn fill(&mut self, token: &ElementToken, text: &str) -> Result<FillOutcome, AdapterError> {
        fill(&mut self.connection, &self.tab, token, text, deadline())
    }

    fn eval(&mut self, expression: &str) -> Value {
        let mut reply = self
            .connection
            .call(
                "Runtime.evaluate",
                json!({ "expression": expression, "returnByValue": true }),
                Some(&self.tab.session_id),
                deadline(),
            )
            .unwrap();
        reply
            .pointer_mut("/result/value")
            .map_or(Value::Null, Value::take)
    }

    fn value(&mut self, id: &str) -> Value {
        self.eval(&format!("document.getElementById('{id}').value"))
    }

    fn take_events(&mut self) -> Value {
        self.eval("window.events.splice(0)")
    }
}

fn open() -> (common::HeadlessChrome, Page) {
    let port = serve(ORDER.to_owned());
    let chrome = launch("value-inputs");
    let mut connection = connect_to(&chrome);
    let tab = common::open_tab(&mut connection, &format!("http://127.0.0.1:{port}/"));
    wait_for_load(&mut connection, &tab, deadline()).unwrap();
    let snapshot = snapshot_page(&mut connection, &tab, deadline()).unwrap();
    (
        chrome,
        Page {
            connection,
            tab,
            snapshot,
        },
    )
}

#[test]
#[ignore = "launches headless Chrome"]
fn each_whole_value_input_is_set_through_one_ref_and_verified() {
    let (_chrome, mut page) = open();
    for (role, name, id, value) in [
        ("timefield", "Delivery time", "delivery", "19:30"),
        ("datefield", "Delivery day", "day", "2026-11-08"),
        ("datefield", "Pickup slot", "slot", "2026-11-08T19:30"),
        ("datefield", "Billing month", "billing", "2026-11"),
        ("datefield", "Roster week", "roster", "2026-W45"),
        ("colorwell", "Box color", "box", "#ff8800"),
    ] {
        let outcome = page.set(role, name, value).unwrap();
        assert_eq!(
            outcome.delivery(),
            DeliverySemantics::delivered_verified(),
            "{id}"
        );
        assert_eq!(outcome.observed.as_deref(), Some(value), "{id}");
        assert_eq!(page.value(id), value, "{id}");
        assert_eq!(
            page.take_events(),
            json!([
                format!("{id}:input:true"),
                format!("{id}:change:true"),
                format!("form:change:{id}")
            ]),
            "{id}"
        );
    }
    let again = page.set("timefield", "Delivery time", "19:30").unwrap();
    assert_eq!(again.delivery(), DeliverySemantics::not_delivered());
    assert_eq!(page.take_events(), json!([]));
    let cleared = page.set("timefield", "Delivery time", "").unwrap();
    assert_eq!(cleared.delivery(), DeliverySemantics::delivered_verified());
    assert_eq!(page.value("delivery"), "");
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_page_that_tracks_the_value_on_the_element_still_sees_the_change() {
    let (_chrome, mut page) = open();
    let outcome = page.set("timefield", "Tracked time", "08:15").unwrap();
    assert_eq!(outcome.delivery(), DeliverySemantics::delivered_verified());
    let events = page.take_events();
    assert!(
        events
            .as_array()
            .unwrap()
            .contains(&json!("tracked:noticed")),
        "{events}"
    );
}

#[test]
#[ignore = "launches headless Chrome"]
fn wrong_or_rejected_values_fail_and_leave_the_input_alone() {
    let (_chrome, mut page) = open();
    page.set("timefield", "Delivery time", "10:00").unwrap();
    page.set("datefield", "Delivery day", "2026-11-08").unwrap();
    page.take_events();

    let other_form = page
        .set("timefield", "Delivery time", "7:30 PM")
        .unwrap_err();
    assert_eq!(other_form.code, ErrorCode::InvalidArgs);
    assert!(other_form.suggestion.unwrap().starts_with("Pass '19:30'."));

    let malformed = page
        .set("datefield", "Delivery day", "11/08/2026")
        .unwrap_err();
    assert_eq!(malformed.code, ErrorCode::InvalidArgs);
    assert!(malformed.suggestion.unwrap().contains("YYYY-MM-DD"));

    let rejected = page
        .set("datefield", "Delivery day", "2026-02-30")
        .unwrap_err();
    assert_eq!(rejected.code, ErrorCode::ActionFailed);
    assert_eq!(rejected.disposition, DeliverySemantics::not_delivered());

    assert_eq!(page.value("delivery"), "10:00");
    assert_eq!(page.value("day"), "2026-11-08");
    assert_eq!(page.take_events(), json!([]));

    let before_min = page.set("datefield", "Delivery day", "2025-06-01").unwrap();
    assert_eq!(
        before_min.delivery(),
        DeliverySemantics::delivered_verified()
    );
    assert_eq!(page.value("day"), "2025-06-01");
}

#[test]
#[ignore = "launches headless Chrome"]
fn one_part_of_a_time_input_is_refused_and_points_at_the_input() {
    let (_chrome, mut page) = open();
    let (_, hours) = page
        .snapshot
        .preorder()
        .into_iter()
        .find(|(node, _)| node.role == "incrementor")
        .map(|(node, origin)| (node.clone(), origin.unwrap().element_token()))
        .expect("the time input shows its parts");

    let error = page.fill(&hours, "07").unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(error.suggestion.unwrap().contains("time input's own ref"));
    assert_eq!(page.value("delivery"), "");
    assert_eq!(page.take_events(), json!([]));
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_range_input_reports_the_value_it_clamped_to() {
    let (_chrome, mut page) = open();
    let (_, spice) = find(&page.snapshot, "slider", "Spice level");

    let set = page.fill(&spice, "7").unwrap();
    assert_eq!(set.delivery(), DeliverySemantics::delivered_verified());

    let clamped = page.fill(&spice, "500").unwrap_err();
    assert_eq!(clamped.code, ErrorCode::ActionFailed);
    assert_eq!(clamped.details.unwrap()["observed"], "10");
    assert_eq!(page.value("spice"), "10");
}
