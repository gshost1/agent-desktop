use super::*;

fn record(pid: i32, window_number: i64, title: Option<&str>) -> WindowRecord {
    WindowRecord {
        app_name: "AgentDeskFixture".into(),
        pid,
        title: title.map(str::to_owned),
        window_number,
        bounds: agent_desktop_core::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        visible: true,
        process_instance: Some(format!("{pid}:1")),
    }
}

fn ax_titles(entries: &[(i64, &str)]) -> FxHashMap<i64, String> {
    entries
        .iter()
        .map(|(number, title)| (*number, (*title).to_owned()))
        .collect()
}

fn fill(records: &mut [WindowRecord], titles: &FxHashMap<i64, String>) {
    fill_missing_titles(records, |_, number| titles.get(&number).map(String::as_str));
}

#[test]
fn core_graphics_title_is_kept_when_accessibility_reports_another() {
    let mut records = vec![record(10, 1, Some("CG Title"))];

    fill(&mut records, &ax_titles(&[(1, "AX Title")]));

    assert_eq!(records[0].display_title(), "CG Title");
}

#[test]
fn missing_core_graphics_title_takes_the_accessibility_title() {
    let mut records = vec![record(10, 1, None)];

    fill(&mut records, &ax_titles(&[(1, "AgentDesk Fixture")]));

    assert_eq!(records[0].display_title(), "AgentDesk Fixture");
    let info = records
        .remove(0)
        .into_window_info(false, None, true)
        .unwrap();
    assert_eq!(info.title, "AgentDesk Fixture");
}

#[test]
fn window_without_any_title_falls_back_to_the_app_name() {
    let mut records = vec![record(10, 1, None)];

    fill(&mut records, &ax_titles(&[(2, "Other Window")]));

    assert_eq!(records[0].display_title(), "AgentDeskFixture");
    let info = records
        .remove(0)
        .into_window_info(false, None, true)
        .unwrap();
    assert_eq!(info.title, "AgentDeskFixture");
}

#[test]
fn empty_accessibility_title_falls_back_to_the_app_name() {
    let mut records = vec![record(10, 1, None), record(10, 2, Some(""))];

    fill(&mut records, &ax_titles(&[(1, ""), (2, "")]));

    assert_eq!(records[0].display_title(), "AgentDeskFixture");
    assert_eq!(records[1].title, None);
    assert_eq!(records[1].display_title(), "AgentDeskFixture");
}

#[test]
fn probes_ask_only_for_untitled_windows_and_skip_failed_owners() {
    let mut records = vec![
        record(10, 1, Some("Titled")),
        record(10, 2, None),
        record(20, 3, None),
    ];
    let mut requested = Vec::new();

    let probes = probe_owners_with(&records, |pid, untitled| {
        requested.push((pid, untitled.clone()));
        if pid == 20 {
            return Err(AdapterError::timeout("busy"));
        }
        Ok(WindowTitleProbe {
            accessible_window_ids: [1, 2].into_iter().collect(),
            title_by_id: ax_titles(&[(1, "AX One"), (2, "AX Two")]),
        })
    });
    fill_from_probes(&mut records, &probes);

    assert_eq!(
        requested,
        vec![
            (10, [2].into_iter().collect()),
            (20, [3].into_iter().collect())
        ]
    );
    assert!(!probes.contains_key(&20));
    assert_eq!(records[0].display_title(), "Titled");
    assert_eq!(records[1].display_title(), "AX Two");
    assert_eq!(records[2].display_title(), "AgentDeskFixture");
}
