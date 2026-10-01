//! Visit history for Back / Forward.

#[derive(Default)]
pub struct History {
  back: Vec<String>,
  forward: Vec<String>,
}

impl History {
  pub fn visit(&mut self, from: Option<&str>, to: &str) {
    if let Some(f) = from {
      if f != to {
        self.back.push(f.to_string());
        self.forward.clear();
      }
    }
  }

  pub fn back(&mut self, current: Option<&str>) -> Option<String> {
    let prev = self.back.pop()?;
    if let Some(c) = current {
      self.forward.push(c.to_string());
    }
    Some(prev)
  }

  pub fn forward(&mut self, current: Option<&str>) -> Option<String> {
    let next = self.forward.pop()?;
    if let Some(c) = current {
      self.back.push(c.to_string());
    }
    Some(next)
  }

  pub fn forget(&mut self, chat: &str) {
    self.back.retain(|c| c != chat);
    self.forward.retain(|c| c != chat);
  }
}

#[cfg(test)]
#[path = "../../tests/nav.rs"]
mod tests;
