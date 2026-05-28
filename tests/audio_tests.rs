use seedrelay::audio::{interleaved_f32_to_mono, pad_frame, AudioConfig, LinearPcmResampler};

#[test]
fn pads_partial_pcm_frame_to_20ms() {
    let config = AudioConfig::default();
    let input = vec![7u8; 10];

    let frame = pad_frame(&input, config);

    assert_eq!(frame.len(), config.bytes_per_frame);
    assert_eq!(&frame[..10], vec![7u8; 10].as_slice());
    assert!(frame[10..].iter().all(|byte| *byte == 0));
}

#[test]
fn averages_interleaved_f32_channels_to_mono() {
    let mono = interleaved_f32_to_mono(&[1.0, -1.0, 0.25, 0.75], 2);

    assert_eq!(mono, vec![0.0, 0.5]);
}

#[test]
fn linear_resampler_converts_mono_f32_to_pcm16_bytes() {
    let mut resampler = LinearPcmResampler::new(4, 2);

    let bytes = resampler.push_mono_f32(&[0.0, 0.5, 1.0, 0.5]);

    assert_eq!(bytes.len(), 4);
    assert_eq!(i16::from_le_bytes([bytes[0], bytes[1]]), 0);
    assert_eq!(i16::from_le_bytes([bytes[2], bytes[3]]), 32767);
}

#[test]
fn linear_resampler_flushes_tail_sample_on_finish() {
    let mut resampler = LinearPcmResampler::new(48_000, 16_000);

    assert!(resampler.push_mono_f32(&[0.5]).is_empty());
    let bytes = resampler.finish();

    assert_eq!(bytes.len(), 2);
    assert_eq!(i16::from_le_bytes([bytes[0], bytes[1]]), 16384);
    assert!(resampler.finish().is_empty());
}
