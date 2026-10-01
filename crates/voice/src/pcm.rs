/// Average interleaved channels into mono.
pub fn mono(data: &[f32], channels: usize) -> Vec<f32> {
  if channels <= 1 {
    return data.to_vec();
  }
  data.chunks(channels).map(|c| c.iter().sum::<f32>() / c.len() as f32).collect()
}

/// Linear-interpolation resampling. Good enough for speech.
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
  if from == to || input.is_empty() {
    return input.to_vec();
  }
  let ratio = from as f64 / to as f64;
  let len = ((input.len() as f64) / ratio).floor() as usize;
  (0..len)
    .map(|i| {
      let pos = i as f64 * ratio;
      let a = pos.floor() as usize;
      let b = (a + 1).min(input.len() - 1);
      let t = (pos - a as f64) as f32;
      input[a] * (1.0 - t) + input[b] * t
    })
    .collect()
}

pub fn to_i16(s: &[f32]) -> Vec<i16> {
  s.iter().map(|v| (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).collect()
}

pub fn from_i16(s: &[i16]) -> Vec<f32> {
  s.iter().map(|v| *v as f32 / i16::MAX as f32).collect()
}

pub fn le_bytes(s: &[i16]) -> Vec<u8> {
  s.iter().flat_map(|v| v.to_le_bytes()).collect()
}

pub fn from_le_bytes(b: &[u8]) -> Vec<i16> {
  b.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect()
}

/// A mono 16-bit PCM WAV file.
pub fn wav(samples: &[i16], rate: u32) -> Vec<u8> {
  let data = le_bytes(samples);
  let mut out = Vec::with_capacity(44 + data.len());
  out.extend_from_slice(b"RIFF");
  out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
  out.extend_from_slice(b"WAVEfmt ");
  out.extend_from_slice(&16u32.to_le_bytes());
  out.extend_from_slice(&1u16.to_le_bytes());
  out.extend_from_slice(&1u16.to_le_bytes());
  out.extend_from_slice(&rate.to_le_bytes());
  out.extend_from_slice(&(rate * 2).to_le_bytes());
  out.extend_from_slice(&2u16.to_le_bytes());
  out.extend_from_slice(&16u16.to_le_bytes());
  out.extend_from_slice(b"data");
  out.extend_from_slice(&(data.len() as u32).to_le_bytes());
  out.extend_from_slice(&data);
  out
}

/// Root-mean-square level, for the input meter.
pub fn level(s: &[f32]) -> f32 {
  if s.is_empty() {
    return 0.0;
  }
  (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt()
}

#[cfg(test)]
#[path = "../tests/pcm.rs"]
mod tests;
