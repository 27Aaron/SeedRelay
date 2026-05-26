use anyhow::{anyhow, Result};
use opus::{Application, Channels, Encoder};

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

pub fn interleaved_f32_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }

    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

pub fn interleaved_i16_to_mono_f32(samples: &[i16], channels: usize) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }

    samples
        .chunks_exact(channels)
        .map(|frame| {
            frame
                .iter()
                .map(|sample| *sample as f32 / i16::MAX as f32)
                .sum::<f32>()
                / channels as f32
        })
        .collect()
}

pub fn interleaved_u16_to_mono_f32(samples: &[u16], channels: usize) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }

    samples
        .chunks_exact(channels)
        .map(|frame| {
            frame
                .iter()
                .map(|sample| (*sample as f32 - 32768.0) / 32768.0)
                .sum::<f32>()
                / channels as f32
        })
        .collect()
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
        Ok(Self { encoder, config })
    }

    pub fn encode(&mut self, pcm_frame: &[u8]) -> Result<Vec<u8>> {
        if pcm_frame.len() != self.config.bytes_per_frame {
            return Err(anyhow!(
                "expected {} bytes per PCM frame, got {}",
                self.config.bytes_per_frame,
                pcm_frame.len()
            ));
        }

        let samples: Vec<i16> = pcm_frame
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        let mut output = vec![0u8; 4000];
        let encoded_len = self
            .encoder
            .encode(&samples, &mut output)
            .map_err(|error| anyhow!("Opus encode failed: {error:?}"))?;
        output.truncate(encoded_len);
        Ok(output)
    }
}
