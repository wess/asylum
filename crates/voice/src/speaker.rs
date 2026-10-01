//! Playback of 24 kHz mono PCM on a dedicated thread. `push` queues audio;
//! `clear` drops what hasn't played (barge-in).

use crate::pcm;
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Speaker {
  queue: Arc<Mutex<VecDeque<f32>>>,
  rate: u32,
  stop: Arc<AtomicBool>,
}

impl Speaker {
  pub fn start() -> Result<Speaker> {
    let queue: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<u32>>();
    let (q, flag) = (queue.clone(), stop.clone());
    std::thread::spawn(move || {
      let run = || -> Result<(cpal::Stream, u32)> {
        let dev = cpal::default_host().default_output_device().ok_or_else(|| anyhow!("No speaker found."))?;
        let cfg = dev.default_output_config()?;
        let rate = cfg.sample_rate();
        let channels = cfg.channels() as usize;
        let config: cpal::StreamConfig = cfg.config();
        let q2 = q.clone();
        let stream = dev.build_output_stream(
          config,
          move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let mut q = q2.lock().expect("audio queue");
            for frame in out.chunks_mut(channels) {
              let v = q.pop_front().unwrap_or(0.0);
              for s in frame {
                *s = v;
              }
            }
          },
          |_| {},
          None,
        )?;
        stream.play()?;
        Ok((stream, rate))
      };
      match run() {
        Ok((stream, rate)) => {
          let _ = ready_tx.send(Ok(rate));
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
    let rate = ready_rx.recv().map_err(|_| anyhow!("speaker thread failed"))??;
    Ok(Speaker { queue, rate, stop })
  }

  /// Queue 24 kHz mono samples.
  pub fn push(&self, samples: &[i16]) {
    let f = pcm::resample(&pcm::from_i16(samples), crate::RATE, self.rate);
    if let Ok(mut q) = self.queue.lock() {
      q.extend(f);
    }
  }

  pub fn clear(&self) {
    if let Ok(mut q) = self.queue.lock() {
      q.clear();
    }
  }

  pub fn playing(&self) -> bool {
    self.queue.lock().map(|q| !q.is_empty()).unwrap_or(false)
  }

  pub fn stop(&self) {
    self.stop.store(true, Ordering::SeqCst);
  }
}
