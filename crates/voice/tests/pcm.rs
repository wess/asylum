use super::*;

#[test]
fn mono_and_resample() {
  assert_eq!(mono(&[1.0, 0.0, 0.5, 0.5], 2), vec![0.5, 0.5]);
  let up = resample(&[0.0, 1.0], 1, 2);
  assert_eq!(up.len(), 4);
  assert!((up[1] - 0.5).abs() < 1e-6);
  assert_eq!(resample(&vec![0.0; 48_000], 48_000, 24_000).len(), 24_000);
}

#[test]
fn i16_round_trip_and_wav_header() {
  let s = to_i16(&[0.0, 1.0, -1.0, 2.0]);
  assert_eq!(s, vec![0, 32767, -32767, 32767]);
  assert_eq!(from_le_bytes(&le_bytes(&s)), s);
  let w = wav(&s, 24_000);
  assert_eq!(&w[..4], b"RIFF");
  assert_eq!(w.len(), 44 + 8);
  assert_eq!(u32::from_le_bytes([w[24], w[25], w[26], w[27]]), 24_000);
}

#[test]
fn level_of_silence_is_zero() {
  assert_eq!(level(&[0.0; 10]), 0.0);
  assert!(level(&[0.5, -0.5]) > 0.49);
}
