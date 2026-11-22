use super::*;
use crate::tree::capabilities::text_units::LengthUnit;

#[test]
fn toolkit_policy_matrix() {
    let rows = [
        (
            Some("gtk"),
            Some("3.24.41"),
            [true, true, true, false, false, true, false],
        ),
        (
            Some("GAIL"),
            Some("2.24.33"),
            [false, false, true, false, false, false, false],
        ),
        (
            Some("GAIL"),
            None,
            [false, false, false, false, false, false, false],
        ),
        (
            Some("GAIL"),
            Some("3.0"),
            [false, false, false, false, false, false, false],
        ),
        (
            Some("GTK"),
            Some("4.14.5"),
            [false, false, true, false, false, false, false],
        ),
        (
            Some("Qt"),
            Some("6.4"),
            [true, true, true, true, false, false, true],
        ),
        (
            Some("Chromium"),
            None,
            [true, false, true, false, false, false, false],
        ),
        (
            None,
            None,
            [false, false, false, false, false, false, false],
        ),
        (
            Some("gtk"),
            None,
            [false, false, false, false, false, false, false],
        ),
        (
            Some("gtk"),
            Some("x"),
            [false, false, false, false, false, false, false],
        ),
        (
            Some("clutter"),
            Some("1.0"),
            [false, false, false, false, false, false, false],
        ),
        (
            Some("Gecko"),
            Some("158.0"),
            [false, false, false, false, true, false, false],
        ),
    ];

    for (name, version, expected) in rows {
        let toolkit = ToolkitId {
            name: name.map(str::to_owned),
            version: version.map(str::to_owned),
        };
        let actual = [
            reports_global_position(&toolkit),
            trusts_same_window_landing(&toolkit),
            trusts_descent(&toolkit),
            insert_length_unit(&toolkit) == LengthUnit::Utf16Units,
            is_gecko(&toolkit),
            combo_selection_is_live(&toolkit),
            combo_name_is_value(&toolkit),
        ];
        assert_eq!(actual, expected, "toolkit {name:?} {version:?}");
    }
}

#[test]
fn gail_two_requires_a_positive_landing_without_gtk_selection_assumptions() {
    let toolkit = ToolkitId {
        name: Some("gail".to_owned()),
        version: Some("2.24.33".to_owned()),
    };
    assert!(trusts_descent(&toolkit));
    assert_eq!(kind(&toolkit), ToolkitKind::Gtk2);
    assert!(!stops_above_target(&toolkit));
    assert!(!selection_indices_are_accessible(&toolkit));
}
