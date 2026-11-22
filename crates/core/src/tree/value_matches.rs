/// Compares text exactly and numeric controls with an absolute readback tolerance.
///
/// Line breaks compare normalised: RichEdit-family controls read a value
/// written with `\n` breaks back with `\r` or `\r\n` breaks (WordPad was seen
/// to refuse its own successful write over them), so `\r\n` and lone `\r`
/// count as `\n` on both sides. The number of breaks still has to agree, so
/// a newline the control dropped or added is never verified; a control's own
/// terminator is removed where it is read, not here.
pub fn value_matches(role: &str, expected: &str, observed: Option<&str>) -> bool {
    let Some(observed) = observed else {
        return false;
    };
    if expected == observed {
        return true;
    }
    if normalize_text(expected) == normalize_text(observed) {
        return true;
    }
    if !matches!(role, "slider" | "incrementor" | "scrollbar" | "handle") {
        return false;
    }
    if let (Ok(expected), Ok(observed)) = (expected.parse::<i128>(), observed.parse::<i128>()) {
        return expected == observed;
    }
    match (expected.parse::<f64>(), observed.parse::<f64>()) {
        (Ok(expected), Ok(observed)) => {
            expected.is_finite()
                && observed.is_finite()
                && expected.abs().max(observed.abs()) < 9_007_199_254_740_992.0
                && (expected - observed).abs() <= 1e-6
        }
        _ => false,
    }
}

/// What an application kept once editing ended: the written value itself, or
/// the same number in the field's own format, such as `0.3000` for `0.3` or
/// `1,200` for `1200`.
pub(crate) fn committed_value_matches(role: &str, expected: &str, observed: Option<&str>) -> bool {
    if value_matches(role, expected, observed) {
        return true;
    }
    let Some(observed) = observed else {
        return false;
    };
    match (plain_number(expected), plain_number(observed)) {
        (Some(expected), Some(observed)) => (expected - observed).abs() <= 1e-6,
        _ => false,
    }
}

fn plain_number(text: &str) -> Option<f64> {
    let digits: String = text
        .trim()
        .chars()
        .filter(|character| !matches!(character, ',' | ' ' | '\u{a0}' | '\u{202f}'))
        .collect();
    digits
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite() && number.abs() < 9_007_199_254_740_992.0)
}

fn normalize_text(value: &str) -> std::borrow::Cow<'_, str> {
    if !value.contains('\r') {
        return std::borrow::Cow::Borrowed(value);
    }
    std::borrow::Cow::Owned(value.replace("\r\n", "\n").replace('\r', "\n"))
}

#[cfg(test)]
mod tests {
    use super::{committed_value_matches, value_matches};

    #[test]
    fn a_committed_field_may_keep_the_written_number_in_its_own_format() {
        for (expected, observed, matches) in [
            ("0.3", "0.3000", true),
            ("1200", "1,200", true),
            ("1200", "1 200", true),
            ("0.3", "0.25", false),
            ("1000000000", "1000000001", false),
            ("abc", "ABC", false),
            ("0.3", "0.3 cm", false),
        ] {
            assert_eq!(
                committed_value_matches("textfield", expected, Some(observed)),
                matches,
                "{expected} -> {observed}"
            );
            assert!(!value_matches("textfield", expected, Some(observed)) || expected == observed);
        }
        assert!(!committed_value_matches("textfield", "0.3", None));
    }

    #[test]
    fn numeric_readback_tolerance_does_not_hide_different_text_or_integer_targets() {
        for (role, expected, observed, matches) in [
            ("textfield", "001", "1", false),
            ("slider", "50", "50.00", true),
            ("slider", "50", "50.0000004", true),
            ("incrementor", "1000000000", "1000000001", false),
            ("incrementor", "1000000000", "1000000001.0", false),
            ("incrementor", "9007199254740993", "9007199254740992", false),
            (
                "incrementor",
                "9007199254740993",
                "9007199254740992.0",
                false,
            ),
            ("slider", "1", "inf", false),
        ] {
            assert_eq!(value_matches(role, expected, Some(observed)), matches);
        }
        assert!(!value_matches("slider", "1", None));
    }

    #[test]
    fn rich_edit_line_breaks_match_plain_line_feeds() {
        for (role, expected, observed, matches) in [
            ("textfield", "a\nb", "a\r\nb", true),
            ("textfield", "a\nb", "a\rb", true),
            ("textfield", "a\nb\nc", "a\r\nb\rc", true),
            ("textfield", "hello 123", "hello 123 ", false),
            ("textfield", "a\nb", "a\nc", false),
            ("textfield", "hello 123", "hello 123\r", false),
            ("textfield", "a\n", "a", false),
            ("textfield", "a", "a\n", false),
            ("textfield", "", "\r", false),
            ("textfield", "", "\n", false),
        ] {
            assert_eq!(
                value_matches(role, expected, Some(observed)),
                matches,
                "expected={expected:?} observed={observed:?}"
            );
        }
        assert!(!value_matches("textfield", "a", None));
    }
}
