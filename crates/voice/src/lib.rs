//! Voice. `pcm` is pure sample math (mixing, resampling, WAV), `mic` and
//! `speaker` are the audio devices on their own threads, `stt` and `tts` are
//! xAI's speech endpoints, and `realtime` is a live voice chat session.

pub mod mic;
pub mod pcm;
pub mod realtime;
pub mod speaker;
pub mod stt;
pub mod tts;

pub const RATE: u32 = 24_000;
pub const BASE: &str = "https://api.x.ai/v1";

pub fn base(custom: &str) -> String {
  if custom.trim().is_empty() {
    BASE.to_string()
  } else {
    custom.trim_end_matches('/').to_string()
  }
}

#[cfg(test)]
#[path = "../tests/speech.rs"]
mod speech;
