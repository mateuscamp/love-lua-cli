//! How the panel writes time, rate and fields: pure functions, no terminal.

use std::time::Duration;

/// A duration for a narrow column: `3s`, `12m`, `5h`, `2d` (rounds down).
pub fn short_duration(seconds: u64) -> String {
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m", seconds / 60),
        3600..86400 => format!("{}h", seconds / 3600),
        _ => format!("{}d", seconds / 86400),
    }
}

/// The running game's clock: `02:14`, or `1:02:14` from one hour on.
pub fn clock(elapsed: Duration) -> String {
    let total = elapsed.as_secs();
    let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

/// The blocks of the rate's sparkline, from level 0 (`▁`) to 7 (`█`); the color of each one comes
/// from `theme`.
pub const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// The level of each window, from 0 (zero facts) to 7 (the maximum of the series).
pub fn levels(counts: &[u32]) -> Vec<usize> {
    let maximum = u64::from(counts.iter().copied().max().unwrap_or(0).max(1));
    counts
        .iter()
        // Rounds up: any fact already lifts the window off level 0.
        .map(|&count| (u64::from(count) * 7).div_ceil(maximum) as usize)
        .collect()
}

/// The fields of a fact on one line: `key value · key value`. A field without a key (the whole
/// loose line, in a `texto` or `erro`) goes as it is; a value longer than 12 characters (a hash, a
/// fingerprint) becomes its first 7 and `…`.
pub fn fields_summary(fields: &[(String, String)]) -> String {
    fields
        .iter()
        .map(|(key, value)| {
            if key.is_empty() {
                value.clone()
            } else {
                format!("{key} {}", shorten(value))
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn shorten(value: &str) -> String {
    if value.chars().count() <= 12 {
        value.to_string()
    } else {
        format!("{}…", value.chars().take(7).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_duration_uses_the_largest_whole_unit() {
        let cases = [
            (0, "0s"),
            (59, "59s"),
            (60, "1m"),
            (3599, "59m"),
            (3600, "1h"),
            (86399, "23h"),
            (86400, "1d"),
        ];
        for (seconds, expected) in cases {
            assert_eq!(short_duration(seconds), expected, "{seconds}");
        }
    }

    #[test]
    fn clock_shows_minutes_and_seconds_and_hours_when_there_are_any() {
        assert_eq!(clock(Duration::from_secs(0)), "00:00");
        assert_eq!(clock(Duration::from_millis(134_900)), "02:14");
        assert_eq!(clock(Duration::from_secs(3734)), "1:02:14");
    }

    #[test]
    fn levels_scale_by_the_maximum_of_the_series() {
        let sparkline = |counts: &[u32]| -> String {
            levels(counts)
                .into_iter()
                .map(|level| BLOCKS[level])
                .collect()
        };
        assert_eq!(levels(&[0, 1, 2, 4]), [0, 2, 4, 7]);
        assert_eq!(sparkline(&[0, 1, 2, 4]), "▁▃▅█");
        assert_eq!(sparkline(&[0, 0, 0]), "▁▁▁");
        assert_eq!(sparkline(&[]), "");
    }

    #[test]
    fn fields_summary_joins_the_fields_and_shortens_the_long_value() {
        let fields = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect()
        };
        assert_eq!(
            fields_summary(&fields(&[
                ("soldado", "2"),
                ("texto", "2º"),
                ("marcado", "sim")
            ])),
            "soldado 2 · texto 2º · marcado sim"
        );
        assert_eq!(
            fields_summary(&fields(&[
                ("", "solto"),
                ("digital", "708171b0098615fe5378")
            ])),
            "solto · digital 708171b…"
        );
        assert_eq!(
            fields_summary(&fields(&[("fonte", "vera_mo_bd.t")])),
            "fonte vera_mo_bd.t",
            "12 characters stay"
        );
    }
}
