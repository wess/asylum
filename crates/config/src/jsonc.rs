//! Strip `//` line and `/* */` block comments from JSON, so a commented
//! `settings.json` parses with plain `serde_json`. Comment characters are
//! replaced with spaces rather than removed so line numbers in any parse error
//! still line up with the original source. String contents are left intact.

/// Return `src` with comments blanked to spaces (newlines preserved).
pub(crate) fn strip(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;

    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        let next = chars.get(i + 1).copied();
        match c {
            '"' => {
                in_string = true;
                out.push('"');
                i += 1;
            }
            '/' if next == Some('/') => {
                while i < chars.len() && chars[i] != '\n' {
                    out.push(' ');
                    i += 1;
                }
            }
            '/' if next == Some('*') => {
                out.push_str("  ");
                i += 2;
                while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                    out.push(if chars[i] == '\n' { '\n' } else { ' ' });
                    i += 1;
                }
                if i < chars.len() {
                    out.push_str("  ");
                    i += 2;
                }
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "../tests/jsonc.rs"]
mod tests;
