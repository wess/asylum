use crate::Template;

pub fn template(kind: Template, title: &str, created: i64) -> String {
    let kind_name = match kind {
        Template::Blank => "note",
        Template::Task => "task",
        Template::Decision => "decision",
        Template::Investigation => "investigation",
        Template::Retrospective => "retrospective",
    };
    let body = match kind {
        Template::Blank => "",
        Template::Task => {
            "## Outcome\n\n## Context\n\n## Constraints\n\n## Acceptance criteria\n\n- [ ] "
        }
        Template::Decision => {
            "## Decision\n\n## Context\n\n## Options considered\n\n## Consequences\n"
        }
        Template::Investigation => "## Question\n\n## Evidence\n\n## Findings\n\n## Next step\n",
        Template::Retrospective => {
            "## Outcome\n\n## What worked\n\n## What did not\n\n## Follow-ups\n\n- [ ] "
        }
    };
    let date = iso_date(created);
    format!(
        "---\ntitle: {title}\ntype: {kind_name}\ncreated: {date}\ntags:\n  - {kind_name}\n---\n\n# {title}\n\n{body}"
    )
}

/// Substitute the supported variables into a user-authored template body.
///
/// Recognized tokens: `{{title}}`, `{{date}}` (YYYY-MM-DD), `{{time}}` (HH:MM),
/// and `{{datetime}}` (YYYY-MM-DD HH:MM). Unknown `{{...}}` tokens are left
/// untouched so a template author sees them and can correct the spelling.
pub fn render_user_template(source: &str, title: &str, created: i64) -> String {
    source
        .replace("{{title}}", title)
        .replace("{{date}}", &iso_date(created))
        .replace("{{time}}", &iso_time(created))
        .replace(
            "{{datetime}}",
            &format!("{} {}", iso_date(created), iso_time(created)),
        )
}

/// Format a Unix timestamp (seconds) as a UTC `YYYY-MM-DD` calendar date.
pub fn iso_date(unix: i64) -> String {
    let (year, month, day) = civil_from_days(unix.div_euclid(86_400));
    format!("{year:04}-{month:02}-{day:02}")
}

/// Format a Unix timestamp (seconds) as a UTC `HH:MM` wall-clock time.
pub fn iso_time(unix: i64) -> String {
    let secs = unix.rem_euclid(86_400);
    format!("{:02}:{:02}", secs / 3600, (secs % 3600) / 60)
}

/// Convert a day count since the Unix epoch into a `(year, month, day)` civil
/// date. Howard Hinnant's `civil_from_days`, valid across the full range.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}
