//! Strip `//` and `/* */` comments and trailing commas so a hand-edited
//! settings file parses as plain JSON. Strings are left untouched.

pub fn strip(text: &str) -> String {
  let bytes = text.as_bytes();
  let mut out = String::with_capacity(text.len());
  let mut i = 0;
  let mut in_str = false;
  while i < bytes.len() {
    let c = bytes[i];
    if in_str {
      out.push(c as char);
      if c == b'\\' && i + 1 < bytes.len() {
        out.push(bytes[i + 1] as char);
        i += 2;
        continue;
      }
      if c == b'"' {
        in_str = false;
      }
      i += 1;
      continue;
    }
    match (c, bytes.get(i + 1)) {
      (b'"', _) => {
        in_str = true;
        out.push('"');
        i += 1;
      }
      (b'/', Some(b'/')) => {
        while i < bytes.len() && bytes[i] != b'\n' {
          i += 1;
        }
      }
      (b'/', Some(b'*')) => {
        i += 2;
        while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
          if bytes[i] == b'\n' {
            out.push('\n');
          }
          i += 1;
        }
        i += 2;
      }
      (b',', _) if next_closes(bytes, i + 1) => i += 1,
      _ => {
        out.push(c as char);
        i += 1;
      }
    }
  }
  // Multi-byte UTF-8 was pushed byte-wise above; rebuild from bytes.
  String::from_utf8(out.chars().map(|c| c as u8).collect()).unwrap_or_default()
}

fn next_closes(bytes: &[u8], mut i: usize) -> bool {
  while i < bytes.len() {
    match bytes[i] {
      b' ' | b'\t' | b'\r' | b'\n' => i += 1,
      b'/' if bytes.get(i + 1) == Some(&b'/') => {
        while i < bytes.len() && bytes[i] != b'\n' {
          i += 1;
        }
      }
      b'/' if bytes.get(i + 1) == Some(&b'*') => {
        i += 2;
        while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
          i += 1;
        }
        i += 2;
      }
      b'}' | b']' => return true,
      _ => return false,
    }
  }
  false
}

#[cfg(test)]
#[path = "../tests/jsonc.rs"]
mod tests;
