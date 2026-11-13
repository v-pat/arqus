//! Pure helpers for picker windows and resume timestamps.

use std::path::Path;
use std::time::UNIX_EPOCH;

/// Rows `[start, end)` to draw so that `selected` stays visible.
pub fn visible_window(selected: usize, len: usize, height: usize) -> (usize, usize) {
    if len == 0 || height == 0 {
        return (0, 0);
    }
    let height = height.min(len);
    let start = if selected < height {
        0
    } else {
        (selected + 1).saturating_sub(height)
    };
    let end = (start + height).min(len);
    (start, end)
}

/// `1970-01-01 00:00` style UTC timestamp (no time zone conversion).
pub fn format_timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        rem / 3600,
        (rem % 3600) / 60
    )
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { y + 1 } else { y }, month, day)
}

/// Modified time of a session file, formatted for the resume picker.
pub fn resume_time(path: &Path) -> String {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| format_timestamp(d.as_secs()))
        .unwrap_or_default()
}

/// `just now`, `5m ago`, `2h ago`, `3d ago` for the sessions picker.
pub fn relative_time(modified: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(modified);
    relative_to(now, modified)
}

/// Pure form of `relative_time`, so the wording can be tested.
pub fn relative_to(now: u64, modified: u64) -> String {
    let secs = now.saturating_sub(modified);
    match secs {
        0..=59 => "just now".to_string(),
        60..=3_599 => format!("{}m ago", secs / 60),
        3_600..=86_399 => format!("{}h ago", secs / 3_600),
        _ => format!("{}d ago", secs / 86_400),
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
