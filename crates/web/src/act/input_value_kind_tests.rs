use super::InputValueKind;

fn canonical(input_type: &str, text: &str) -> Option<String> {
    InputValueKind::from_input_type(input_type)
        .unwrap()
        .canonical(text)
}

#[test]
fn each_type_keeps_a_value_already_in_its_own_format() {
    for (input_type, text) in [
        ("date", "2026-11-08"),
        ("time", "19:30"),
        ("time", "07:05:09"),
        ("time", "07:05:09.250"),
        ("datetime-local", "2026-11-08T19:30"),
        ("month", "2026-11"),
        ("week", "2026-W45"),
        ("color", "#ff8800"),
        ("range", "70"),
        ("range", "0.5"),
    ] {
        assert_eq!(
            canonical(input_type, text).as_deref(),
            Some(text),
            "{input_type} {text}"
        );
    }
}

#[test]
fn an_unambiguous_other_form_maps_to_the_format_the_browser_reports() {
    for (input_type, text, expected) in [
        ("date", "2026-1-5", "2026-01-05"),
        ("time", "7:30 PM", "19:30"),
        ("time", "7:30pm", "19:30"),
        ("time", "12:05 AM", "00:05"),
        ("time", "12:05 PM", "12:05"),
        ("time", "7:30", "07:30"),
        ("datetime-local", "2026-11-08 19:30", "2026-11-08T19:30"),
        ("datetime-local", "2026-11-08T19:30:00", "2026-11-08T19:30"),
        (
            "datetime-local",
            "2026-11-08T19:30:00.000",
            "2026-11-08T19:30",
        ),
        (
            "datetime-local",
            "2026-11-08T19:30:05.500",
            "2026-11-08T19:30:05.5",
        ),
        ("month", "2026-3", "2026-03"),
        ("week", "2026-w7", "2026-W07"),
        ("color", "#F80", "#ff8800"),
        ("color", "#FF8800", "#ff8800"),
        ("range", "70.0", "70"),
        ("range", "-0", "0"),
    ] {
        assert_eq!(
            canonical(input_type, text).as_deref(),
            Some(expected),
            "{input_type} {text}"
        );
    }
}

#[test]
fn ambiguous_or_malformed_text_has_no_value() {
    for (input_type, text) in [
        ("date", "11/08/2026"),
        ("date", "08.11.2026"),
        ("date", "Nov 8 2026"),
        ("date", "2026-13-01"),
        ("date", "2026-11-08T19:30"),
        ("date", "26-11-08"),
        ("time", "25:00"),
        ("time", "19:3"),
        ("time", "13:00 PM"),
        ("time", "0:30 AM"),
        ("time", "19:30:60"),
        ("time", "19:30:00.1234"),
        ("time", "1930"),
        ("time", "07"),
        ("time", "PM"),
        ("datetime-local", "2026-11-08"),
        ("datetime-local", "19:30"),
        ("month", "November 2026"),
        ("week", "2026-45"),
        ("week", "2026-W54"),
        ("color", "red"),
        ("color", "#ff88"),
        ("range", "loud"),
        ("range", "NaN"),
        ("range", "inf"),
    ] {
        assert_eq!(canonical(input_type, text), None, "{input_type} {text}");
    }
}

#[test]
fn empty_text_clears_a_date_or_time_input_but_not_a_color_or_range() {
    for input_type in ["date", "time", "datetime-local", "month", "week"] {
        assert_eq!(canonical(input_type, "").as_deref(), Some(""));
    }
    for input_type in ["color", "range"] {
        assert_eq!(canonical(input_type, ""), None);
    }
}

#[test]
fn text_input_types_are_not_assigned_whole() {
    for input_type in ["text", "number", "email", "checkbox", ""] {
        assert_eq!(InputValueKind::from_input_type(input_type), None);
    }
}
