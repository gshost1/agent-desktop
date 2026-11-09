use super::input_value_text::{color, date, date_time_local, month, number, time, week};

/// An HTML input type whose value is assigned whole. Chromium draws these
/// inputs with their own editing UI, so text typed or inserted into them
/// never reaches their value; the value string has one format per type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputValueKind {
    Date,
    Time,
    DateTimeLocal,
    Month,
    Week,
    Color,
    Range,
}

impl InputValueKind {
    pub(crate) fn from_input_type(input_type: &str) -> Option<Self> {
        Some(match input_type {
            "date" => Self::Date,
            "time" => Self::Time,
            "datetime-local" => Self::DateTimeLocal,
            "month" => Self::Month,
            "week" => Self::Week,
            "color" => Self::Color,
            "range" => Self::Range,
            _ => return None,
        })
    }

    pub(crate) fn input_type(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Time => "time",
            Self::DateTimeLocal => "datetime-local",
            Self::Month => "month",
            Self::Week => "week",
            Self::Color => "color",
            Self::Range => "range",
        }
    }

    /// The value format the input takes and reports, in words.
    pub(crate) fn format(self) -> &'static str {
        match self {
            Self::Date => "YYYY-MM-DD",
            Self::Time => "HH:MM in 24-hour time, with optional :SS",
            Self::DateTimeLocal => "YYYY-MM-DDTHH:MM",
            Self::Month => "YYYY-MM",
            Self::Week => "YYYY-Www",
            Self::Color => "#rrggbb in lowercase hex",
            Self::Range => "a number between the input's min and max",
        }
    }

    pub(crate) fn example(self) -> &'static str {
        match self {
            Self::Date => "2026-11-08",
            Self::Time => "19:30",
            Self::DateTimeLocal => "2026-11-08T19:30",
            Self::Month => "2026-11",
            Self::Week => "2026-W45",
            Self::Color => "#ff8800",
            Self::Range => "50",
        }
    }

    /// `text` in the input's own format, the form the browser reports back,
    /// or `None` when `text` is not a value of this type. Empty text clears
    /// a date or time input; a color or range input always holds a value.
    pub(crate) fn canonical(self, text: &str) -> Option<String> {
        if text.is_empty() {
            return (!matches!(self, Self::Color | Self::Range)).then(String::new);
        }
        match self {
            Self::Date => date(text),
            Self::Time => time(text),
            Self::DateTimeLocal => date_time_local(text),
            Self::Month => month(text),
            Self::Week => week(text),
            Self::Color => color(text),
            Self::Range => number(text),
        }
    }
}

#[cfg(test)]
#[path = "input_value_kind_tests.rs"]
mod tests;
