//! Server-sent-event framing. Bytes arrive in arbitrary chunks; `Decoder`
//! buffers them and yields the payload of each complete `data:` line.

#[derive(Default)]
pub struct Decoder {
  buf: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frame {
  Data(String),
  Done,
}

impl Decoder {
  pub fn push(&mut self, bytes: &[u8]) -> Vec<Frame> {
    self.buf.push_str(&String::from_utf8_lossy(bytes));
    let mut out = Vec::new();
    while let Some(pos) = self.buf.find('\n') {
      let line: String = self.buf.drain(..=pos).collect();
      if let Some(frame) = parse_line(line.trim_end_matches(['\r', '\n'])) {
        out.push(frame);
      }
    }
    out
  }
}

fn parse_line(line: &str) -> Option<Frame> {
  let data = line.strip_prefix("data:")?.trim_start();
  if data == "[DONE]" {
    return Some(Frame::Done);
  }
  (!data.is_empty()).then(|| Frame::Data(data.to_string()))
}

#[cfg(test)]
#[path = "../tests/sse.rs"]
mod tests;
