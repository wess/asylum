//! Keep tool output within what a model context can afford: head and tail of
//! long text with the middle elided, cut on char boundaries.

pub fn clip(text: &str, max: usize) -> String {
  if text.len() <= max {
    return text.to_string();
  }
  let head = floor(text, max * 2 / 3);
  let tail_start = ceil(text, text.len() - (max - head.len()));
  format!(
    "{}\n\n… [{} bytes elided] …\n\n{}",
    &text[..head.len()],
    tail_start - head.len(),
    &text[tail_start..]
  )
}

fn floor(s: &str, mut i: usize) -> &str {
  while !s.is_char_boundary(i) {
    i -= 1;
  }
  &s[..i]
}

fn ceil(s: &str, mut i: usize) -> usize {
  while !s.is_char_boundary(i) {
    i += 1;
  }
  i
}

#[cfg(test)]
#[path = "../tests/clip.rs"]
mod tests;
