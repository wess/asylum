//! Tool calls stream as indexed fragments: the id and name arrive once, the
//! JSON arguments arrive in pieces. `Accumulator` stitches them back together.

use crate::types::{CallDelta, ToolCall};

#[derive(Default)]
pub struct Accumulator {
  calls: Vec<ToolCall>,
}

impl Accumulator {
  pub fn push(&mut self, delta: &CallDelta) {
    while self.calls.len() <= delta.index {
      self.calls.push(ToolCall::new("", "", ""));
    }
    let call = &mut self.calls[delta.index];
    if let Some(id) = &delta.id {
      call.id.push_str(id);
    }
    if let Some(f) = &delta.function {
      if let Some(name) = &f.name {
        call.function.name.push_str(name);
      }
      if let Some(args) = &f.arguments {
        call.function.arguments.push_str(args);
      }
    }
  }

  pub fn finish(self) -> Vec<ToolCall> {
    self
      .calls
      .into_iter()
      .enumerate()
      .filter(|(_, c)| !c.function.name.is_empty())
      .map(|(i, mut c)| {
        if c.id.is_empty() {
          c.id = format!("call_{i}");
        }
        if c.function.arguments.trim().is_empty() {
          c.function.arguments = "{}".into();
        }
        c
      })
      .collect()
  }
}

#[cfg(test)]
#[path = "../tests/accumulate.rs"]
mod tests;
