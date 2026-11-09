//! Parsers for the value strings of HTML inputs that take their value whole.
//! Each returns the value in the input's own format, as the browser reports
//! it back, or `None` when the text is not a value of that kind.

pub(crate) fn date(text: &str) -> Option<String> {
    let mut parts = text.split('-');
    let year = year(parts.next()?)?;
    let month = bounded(parts.next()?, 1..=2, 1..=12)?;
    let day = bounded(parts.next()?, 1..=2, 1..=31)?;
    parts
        .next()
        .is_none()
        .then(|| format!("{year}-{month:02}-{day:02}"))
}

pub(crate) fn month(text: &str) -> Option<String> {
    let (year_text, month_text) = text.split_once('-')?;
    let year = year(year_text)?;
    let month = bounded(month_text, 1..=2, 1..=12)?;
    Some(format!("{year}-{month:02}"))
}

pub(crate) fn week(text: &str) -> Option<String> {
    let (year_text, week_text) = text.split_once('-')?;
    let year = year(year_text)?;
    let number = week_text
        .strip_prefix('W')
        .or_else(|| week_text.strip_prefix('w'))?;
    let number = bounded(number, 1..=2, 1..=53)?;
    Some(format!("{year}-W{number:02}"))
}

/// A time of day as `HH:MM`, keeping any seconds and fraction the text
/// gives. A trailing `AM` or `PM` reads the hour on a 12-hour clock.
pub(crate) fn time(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let lower = trimmed.to_ascii_lowercase();
    let (clock, afternoon) = match lower.strip_suffix("pm") {
        Some(clock) => (clock.trim_end(), Some(true)),
        None => match lower.strip_suffix("am") {
            Some(clock) => (clock.trim_end(), Some(false)),
            None => (lower.as_str(), None),
        },
    };
    let mut parts = clock.split(':');
    let hour_text = parts.next()?;
    let minute = bounded(parts.next()?, 2..=2, 0..=59)?;
    let seconds = match parts.next() {
        Some(text) => Some(seconds(text)?),
        None => None,
    };
    if parts.next().is_some() {
        return None;
    }
    let hour = match afternoon {
        Some(afternoon) => bounded(hour_text, 1..=2, 1..=12)? % 12 + if afternoon { 12 } else { 0 },
        None => bounded(hour_text, 1..=2, 0..=23)?,
    };
    let mut value = format!("{hour:02}:{minute:02}");
    if let Some(seconds) = seconds {
        value.push(':');
        value.push_str(&seconds);
    }
    Some(value)
}

/// A local date and time as the browser normalizes it: a `T` between the
/// date and the time, and the seconds and fraction left out when they are
/// zero.
pub(crate) fn date_time_local(text: &str) -> Option<String> {
    let split = text.find(['T', 't', ' '])?;
    let date = date(&text[..split])?;
    let time = time(&text[split + 1..])?;
    Some(format!("{date}T{}", shortest_time(&time)))
}

/// A color as `#rrggbb` in lowercase hex; `#rgb` doubles each digit.
pub(crate) fn color(text: &str) -> Option<String> {
    let hex = text.trim().strip_prefix('#')?;
    if !hex.chars().all(|digit| digit.is_ascii_hexdigit()) {
        return None;
    }
    let hex = hex.to_ascii_lowercase();
    match hex.len() {
        6 => Some(format!("#{hex}")),
        3 => Some(hex.chars().fold(String::from("#"), |mut value, digit| {
            value.push(digit);
            value.push(digit);
            value
        })),
        _ => None,
    }
}

/// A finite number in the shortest form the browser writes it back in.
pub(crate) fn number(text: &str) -> Option<String> {
    let value: f64 = text.trim().parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    let value = if value == 0.0 { 0.0 } else { value };
    Some(format!("{value}"))
}

fn shortest_time(time: &str) -> String {
    let Some((clock, seconds)) = time.rsplit_once(':').filter(|_| time.len() > 5) else {
        return time.to_owned();
    };
    let (whole, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
    let fraction = fraction.trim_end_matches('0');
    match (whole, fraction) {
        ("00", "") => clock.to_owned(),
        (_, "") => format!("{clock}:{whole}"),
        _ => format!("{clock}:{whole}.{fraction}"),
    }
}

fn seconds(text: &str) -> Option<String> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    bounded(whole, 2..=2, 0..=59)?;
    if text.contains('.')
        && (fraction.is_empty()
            || fraction.len() > 3
            || !fraction.chars().all(|digit| digit.is_ascii_digit()))
    {
        return None;
    }
    Some(text.to_owned())
}

fn year(text: &str) -> Option<&str> {
    bounded(text, 4..=6, 1..=275_760).map(|_| text)
}

fn bounded(
    text: &str,
    digits: std::ops::RangeInclusive<usize>,
    range: std::ops::RangeInclusive<u32>,
) -> Option<u32> {
    if !digits.contains(&text.len()) || !text.chars().all(|digit| digit.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().filter(|value| range.contains(value))
}
