use super::*;

fn combo(key: &str, modifiers: &[Modifier]) -> KeyCombo {
    KeyCombo {
        key: key.into(),
        modifiers: modifiers.to_vec(),
    }
}

fn edges(steps: &[Step]) -> Vec<(u8, bool)> {
    steps
        .iter()
        .filter_map(|step| match step {
            Step::Key { code, down } => Some((*code, *down)),
            Step::Pause(_) => None,
            _ => unreachable!(),
        })
        .collect()
}

#[test]
fn modified_chords_settle_once_before_the_base_key() {
    let map = Keymap::sample();
    for (key, modifiers, down_count) in [
        ("a", vec![Modifier::Ctrl], 1),
        (
            "a",
            vec![Modifier::Ctrl, Modifier::Ctrl, Modifier::Shift],
            2,
        ),
        ("A", vec![], 1),
        ("A", vec![Modifier::Shift], 1),
    ] {
        let plan = plan_combo(&map, &combo(key, &modifiers)).unwrap();
        assert_eq!(
            plan.steps[down_count],
            Step::Pause(Duration::from_millis(10))
        );
        assert_eq!(plan.steps.len(), 2 * down_count + 3);
        assert!(
            plan.steps[..down_count]
                .iter()
                .all(|step| matches!(step, Step::Key { down: true, .. }))
        );
        assert_eq!(
            plan.steps[down_count + 1],
            Step::Key {
                code: 10,
                down: true
            }
        );
        assert_eq!(
            plan.steps[down_count + 2],
            Step::Key {
                code: 10,
                down: false
            }
        );
        let downs = edges(&plan.steps[..down_count]);
        let releases = edges(&plan.steps[down_count + 3..]);
        assert_eq!(
            releases,
            downs
                .into_iter()
                .rev()
                .map(|(code, _)| (code, false))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn unmodified_key_has_no_pause() {
    let plan = plan_combo(&Keymap::sample(), &combo("a", &[])).unwrap();
    assert_eq!(
        plan.steps,
        vec![
            Step::Key {
                code: 10,
                down: true
            },
            Step::Key {
                code: 10,
                down: false
            }
        ]
    );
}

#[test]
fn invalid_chords_remain_not_delivered() {
    let map = Keymap::new(8, 2, vec![0x61, 0x41], 0, Vec::new());
    for chord in [
        combo("", &[]),
        combo("你", &[]),
        combo("a", &[Modifier::Alt]),
        combo("A", &[]),
    ] {
        let error = plan_combo(&map, &chord).unwrap_err();
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    }
}

#[test]
fn combo_delivery_and_refusals() {
    let map = Keymap::sample();
    let plan = plan_combo(
        &map,
        &combo("a", &[Modifier::Ctrl, Modifier::Ctrl, Modifier::Shift]),
    )
    .unwrap();
    assert_eq!(
        edges(&plan.steps),
        [
            (9, true),
            (8, true),
            (10, true),
            (10, false),
            (8, false),
            (9, false)
        ]
    );
    let inferred = plan_combo(&map, &combo("A", &[])).unwrap();
    let explicit = plan_combo(&map, &combo("A", &[Modifier::Shift])).unwrap();
    assert_eq!(edges(&inferred.steps), edges(&explicit.steps));
    assert_eq!(
        edges(&inferred.steps)
            .iter()
            .filter(|(code, down)| *code == 8 && *down)
            .count(),
        1
    );
    assert_eq!(
        plan_combo(&map, &combo("你", &[])).unwrap_err().code,
        ErrorCode::InvalidArgs
    );
    let bare = Keymap::new(8, 2, vec![0x61, 0x41], 0, Vec::new());
    assert_eq!(
        plan_combo(&bare, &combo("a", &[Modifier::Ctrl]))
            .unwrap_err()
            .code,
        ErrorCode::ActionNotSupported
    );
    let steps = plan_keysyms(&combo(
        "a",
        &[Modifier::Ctrl, Modifier::Alt, Modifier::Ctrl],
    ))
    .unwrap();
    assert_eq!(
        steps.iter().map(|s| (s.keysym, s.down)).collect::<Vec<_>>(),
        [
            (0xffe3, true),
            (0xffe9, true),
            (0x61, true),
            (0x61, false),
            (0xffe9, false),
            (0xffe3, false)
        ]
    );
}
