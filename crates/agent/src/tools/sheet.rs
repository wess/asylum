//! The Bot message a turn is writing: its body and cards, persisted on
//! every change so the UI (and a crash) always sees the latest.

use crate::event::Event;
use crate::part::{self, Part};
use crate::runtime::Runtime;
use anyhow::Result;
use std::sync::Mutex;

pub struct Sheet {
  pub chat: String,
  pub message: String,
  pub body: Mutex<String>,
  pub parts: Mutex<Vec<Part>>,
  pub status: Mutex<String>,
}

impl Sheet {
  pub fn new(chat: &str, message: &str) -> Self {
    Self {
      chat: chat.into(),
      message: message.into(),
      body: Mutex::new(String::new()),
      parts: Mutex::new(Vec::new()),
      status: Mutex::new(store::messages::STREAMING.into()),
    }
  }

  pub fn text(&self) -> String {
    self.body.lock().map(|b| b.clone()).unwrap_or_default()
  }

  pub fn append(&self, t: &str) {
    if let Ok(mut b) = self.body.lock() {
      b.push_str(t);
    }
  }

  pub fn set_text(&self, t: &str) {
    if let Ok(mut b) = self.body.lock() {
      *b = t.to_string();
    }
  }

  pub fn push(&self, p: Part) -> usize {
    let mut parts = self.parts.lock().expect("parts");
    parts.push(p);
    parts.len() - 1
  }

  pub fn set(&self, i: usize, p: Part) {
    if let Ok(mut parts) = self.parts.lock() {
      if i < parts.len() {
        parts[i] = p;
      }
    }
  }

  pub fn get(&self, i: usize) -> Option<Part> {
    self.parts.lock().ok()?.get(i).cloned()
  }

  pub fn all(&self) -> Vec<Part> {
    self.parts.lock().map(|p| p.clone()).unwrap_or_default()
  }

  pub fn set_status(&self, s: &str) {
    if let Ok(mut st) = self.status.lock() {
      *st = s.to_string();
    }
  }

  pub async fn save(&self, rt: &Runtime) -> Result<()> {
    let body = self.text();
    let parts = part::to_values(&self.all());
    let status = self.status.lock().map(|s| s.clone()).unwrap_or_default();
    store::messages::update(&rt.pool, &self.message, &body, &parts, &status).await?;
    rt.emit(Event::Message { chat: self.chat.clone(), message: self.message.clone() });
    Ok(())
  }
}
