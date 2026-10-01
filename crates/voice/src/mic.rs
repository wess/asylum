//! Microphone capture on a dedicated thread (audio streams are not Send on
//! every platform). Emits mono f32 chunks resampled to 24 kHz.

use crate::pcm;
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct Mic {
  stop: Arc<AtomicBool>,
  pub rx: mpsc::UnboundedReceiver<Vec<f32>>,
}

impl Drop for Mic {
  fn drop(&mut self) {
    self.stop.store(true, Ordering::SeqCst);
  }
}

/// Input device names, default first.
pub fn devices() -> Vec<String> {
  let host = cpal::default_host();
  let default = host.default_input_device().and_then(|d| d.description().ok()).map(|d| d.name().to_string());
  let mut names: Vec<String> = host
    .input_devices()
    .map(|ds| ds.filter_map(|d| d.description().ok().map(|x| x.name().to_string())).collect())
    .unwrap_or_default();
  if let Some(d) = default {
    names.retain(|n| n != &d);
    names.insert(0, d);
  }
  names
}

fn pick(name: &str) -> Option<cpal::Device> {
  let host = cpal::default_host();
  if !name.is_empty() {
    if let Ok(mut ds) = host.input_devices() {
      if let Some(d) = ds.find(|d| d.description().map(|x| x.name() == name).unwrap_or(false)) {
        return Some(d);
      }
    }
  }
  host.default_input_device()
}

impl Mic {
  pub fn start(device: &str) -> Result<Mic> {
    let (tx, rx) = mpsc::unbounded_channel();
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<()>>();
    let flag = stop.clone();
    let name = device.to_string();
    std::thread::spawn(move || {
      let run = || -> Result<cpal::Stream> {
        let dev = pick(&name).ok_or_else(|| anyhow!("No microphone found."))?;
        let cfg = dev.default_input_config()?;
        let rate = cfg.sample_rate();
        let channels = cfg.channels() as usize;
        let config: cpal::StreamConfig = cfg.config();
        let tx2 = tx.clone();
        let stream = match cfg.sample_format() {
          cpal::SampleFormat::F32 => dev.build_input_stream(
            config,
            move |d: &[f32], _: &cpal::InputCallbackInfo| {
              let _ = tx2.send(pcm::resample(&pcm::mono(d, channels), rate, crate::RATE));
            },
            |_| {},
            None,
          )?,
          cpal::SampleFormat::I16 => dev.build_input_stream(
            config,
            move |d: &[i16], _: &cpal::InputCallbackInfo| {
              let f = pcm::from_i16(d);
              let _ = tx2.send(pcm::resample(&pcm::mono(&f, channels), rate, crate::RATE));
            },
            |_| {},
            None,
          )?,
          f => return Err(anyhow!("unsupported microphone format {f:?}")),
        };
        stream.play()?;
        Ok(stream)
      };
      match run() {
        Ok(stream) => {
          let _ = ready_tx.send(Ok(()));
          while !flag.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(50));
          }
          drop(stream);
        }
        Err(e) => {
          let _ = ready_tx.send(Err(e));
        }
      }
    });
    ready_rx.recv().map_err(|_| anyhow!("microphone thread failed"))??;
    Ok(Mic { stop, rx })
  }
}
