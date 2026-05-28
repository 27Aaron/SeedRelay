use anyhow::{anyhow, Result};
use opus::{Application, Channels, Encoder};

const OPUS_MAX_FRAME_BYTES: usize = 4000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub frame_duration_ms: u32,
    pub samples_per_frame: usize,
    pub bytes_per_frame: usize,
}

impl AudioConfig {
    pub fn new(sample_rate: u32, channels: u16, frame_duration_ms: u32) -> Self {
        let samples_per_frame = (sample_rate * frame_duration_ms / 1000) as usize;
        let bytes_per_frame = samples_per_frame * channels as usize * 2;
        Self {
            sample_rate,
            channels,
            frame_duration_ms,
            samples_per_frame,
            bytes_per_frame,
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self::new(16_000, 1, 20)
    }
}

pub fn pad_frame(frame: &[u8], config: AudioConfig) -> Vec<u8> {
    let mut padded = vec![0u8; config.bytes_per_frame];
    let copy_len = frame.len().min(config.bytes_per_frame);
    padded[..copy_len].copy_from_slice(&frame[..copy_len]);
    padded
}

pub fn silence_frame(config: AudioConfig) -> Vec<u8> {
    vec![0u8; config.bytes_per_frame]
}

pub struct LinearPcmResampler {
    input_sample_rate: u32,
    output_sample_rate: u32,
    buffer: Vec<f32>,
    cursor: f64,
}

impl LinearPcmResampler {
    pub fn new(input_sample_rate: u32, output_sample_rate: u32) -> Self {
        Self {
            input_sample_rate,
            output_sample_rate,
            buffer: Vec::new(),
            cursor: 0.0,
        }
    }

    pub fn push_mono_f32(&mut self, mono_samples: &[f32]) -> Vec<u8> {
        self.buffer.extend_from_slice(mono_samples);
        if self.buffer.len() < 2 {
            return Vec::new();
        }

        let step = self.input_sample_rate as f64 / self.output_sample_rate as f64;
        let mut pcm = Vec::new();

        while (self.cursor.floor() as usize) + 1 < self.buffer.len() {
            let index = self.cursor.floor() as usize;
            let fraction = (self.cursor - index as f64) as f32;
            let current = self.buffer[index];
            let next = self.buffer[index + 1];
            let sample = current + (next - current) * fraction;
            let sample = f32_to_i16(sample);
            pcm.extend_from_slice(&sample.to_le_bytes());
            self.cursor += step;
        }

        let consumed = self.cursor.floor() as usize;
        if consumed > 0 {
            let drain_to = consumed.min(self.buffer.len().saturating_sub(1));
            self.buffer.drain(..drain_to);
            self.cursor -= drain_to as f64;
        }

        pcm
    }

    pub fn finish(&mut self) -> Vec<u8> {
        if self.buffer.is_empty() {
            return Vec::new();
        }

        let index = (self.cursor.floor() as usize).min(self.buffer.len() - 1);
        let sample = f32_to_i16(self.buffer[index]);
        self.buffer.clear();
        self.cursor = 0.0;
        sample.to_le_bytes().to_vec()
    }
}

fn f32_to_i16(sample: f32) -> i16 {
    let sample = sample.clamp(-1.0, 1.0);
    if sample >= 0.0 {
        (sample * i16::MAX as f32).round() as i16
    } else {
        (sample * 32768.0).round() as i16
    }
}

pub struct OpusFrameEncoder {
    encoder: Encoder,
    config: AudioConfig,
    samples: Vec<i16>,
    output: Vec<u8>,
}

impl OpusFrameEncoder {
    pub fn new(config: AudioConfig) -> Result<Self> {
        let channels = match config.channels {
            1 => Channels::Mono,
            2 => Channels::Stereo,
            other => return Err(anyhow!("unsupported channel count: {other}")),
        };
        let encoder = Encoder::new(config.sample_rate, channels, Application::Audio)
            .map_err(|error| anyhow!("failed to create Opus encoder: {error:?}"))?;
        Ok(Self {
            encoder,
            config,
            samples: Vec::with_capacity(config.samples_per_frame * config.channels as usize),
            output: vec![0u8; OPUS_MAX_FRAME_BYTES],
        })
    }

    pub fn encode(&mut self, pcm_frame: &[u8]) -> Result<Vec<u8>> {
        if pcm_frame.len() != self.config.bytes_per_frame {
            return Err(anyhow!(
                "expected {} bytes per PCM frame, got {}",
                self.config.bytes_per_frame,
                pcm_frame.len()
            ));
        }

        self.samples.clear();
        self.samples.extend(
            pcm_frame
                .chunks_exact(2)
                .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]])),
        );
        let encoded_len = self
            .encoder
            .encode(&self.samples, &mut self.output)
            .map_err(|error| anyhow!("Opus encode failed: {error:?}"))?;
        Ok(self.output[..encoded_len].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_encoder_reuses_internal_encode_buffers() {
        let config = AudioConfig::default();
        let mut encoder = OpusFrameEncoder::new(config).expect("encoder");
        let frame = silence_frame(config);

        let first = encoder.encode(&frame).expect("first encode");
        let sample_capacity = encoder.samples.capacity();
        let output_len = encoder.output.len();
        let second = encoder.encode(&frame).expect("second encode");

        assert!(!first.is_empty());
        assert!(!second.is_empty());
        assert_eq!(encoder.samples.capacity(), sample_capacity);
        assert_eq!(encoder.output.len(), output_len);
    }
}
