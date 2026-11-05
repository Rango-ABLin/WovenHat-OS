//! Fixed-capacity PCM capture and mixing primitives.
//!
//! Device drivers remain responsible for DMA and capability checks. This
//! module owns only caller-buffered signed 16-bit PCM: no thread, pointer, or
//! device state crosses the boundary.

pub const MAX_CHANNELS: usize = 2;
pub const MAX_MIX_SAMPLES: usize = 4096;
pub const MAX_CAPTURE_SAMPLES: usize = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PcmFormat {
    pub sample_rate: u32,
    pub channels: u8,
}

impl PcmFormat {
    pub const STEREO_48_KHZ: Self = Self {
        sample_rate: 48_000,
        channels: 2,
    };
    pub fn valid(self) -> bool {
        self.sample_rate != 0 && self.channels != 0 && usize::from(self.channels) <= MAX_CHANNELS
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidFormat,
    Misaligned,
    TooLarge,
    Empty,
}

/// Saturating interleaved S16 mixer. Each source is mixed into a bounded
/// accumulator, avoiding wraparound distortion on adversarial samples.
pub struct Mixer {
    format: PcmFormat,
    samples: [i32; MAX_MIX_SAMPLES],
    sample_count: usize,
}

impl Mixer {
    pub const fn new(format: PcmFormat) -> Self {
        Self {
            format,
            samples: [0; MAX_MIX_SAMPLES],
            sample_count: 0,
        }
    }
    pub const fn format(&self) -> PcmFormat {
        self.format
    }
    pub fn clear(&mut self) {
        self.samples = [0; MAX_MIX_SAMPLES];
        self.sample_count = 0;
    }
    pub fn mix_s16le(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        if !self.format.valid() {
            return Err(Error::InvalidFormat);
        }
        if bytes.is_empty() {
            return Err(Error::Empty);
        }
        if !bytes.len().is_multiple_of(2) {
            return Err(Error::Misaligned);
        }
        let count = bytes.len() / 2;
        if !count.is_multiple_of(usize::from(self.format.channels)) {
            return Err(Error::Misaligned);
        }
        if count > MAX_MIX_SAMPLES || count > self.samples.len() {
            return Err(Error::TooLarge);
        }
        if self.sample_count != 0 && self.sample_count != count {
            return Err(Error::TooLarge);
        }
        for (index, chunk) in bytes.as_chunks::<2>().0.iter().enumerate() {
            self.samples[index] = self.samples[index]
                .saturating_add(i32::from(i16::from_le_bytes([chunk[0], chunk[1]])));
        }
        self.sample_count = count;
        Ok(count / usize::from(self.format.channels))
    }
    pub fn render_s16le(&self, output: &mut [u8]) -> Result<usize, Error> {
        if self.sample_count == 0 {
            return Err(Error::Empty);
        }
        if output.len() < self.sample_count * 2 {
            return Err(Error::TooLarge);
        }
        for (index, sample) in self.samples[..self.sample_count].iter().enumerate() {
            let value = (*sample).clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
            output[index * 2..index * 2 + 2].copy_from_slice(&value.to_le_bytes());
        }
        Ok(self.sample_count * 2)
    }
}

/// Bounded capture FIFO. Overflow drops the oldest sample and records it,
/// ensuring a stalled consumer cannot exhaust kernel or application memory.
pub struct CaptureRing {
    format: PcmFormat,
    samples: [i16; MAX_CAPTURE_SAMPLES],
    read: usize,
    write: usize,
    len: usize,
    dropped_samples: u64,
}

impl CaptureRing {
    pub const fn new(format: PcmFormat) -> Self {
        Self {
            format,
            samples: [0; MAX_CAPTURE_SAMPLES],
            read: 0,
            write: 0,
            len: 0,
            dropped_samples: 0,
        }
    }
    pub const fn dropped_samples(&self) -> u64 {
        self.dropped_samples
    }
    pub const fn available_samples(&self) -> usize {
        self.len
    }
    pub fn push_s16le(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        if !self.format.valid() {
            return Err(Error::InvalidFormat);
        }
        if !bytes.len().is_multiple_of(2) {
            return Err(Error::Misaligned);
        }
        let samples = bytes.len() / 2;
        if !samples.is_multiple_of(usize::from(self.format.channels)) {
            return Err(Error::Misaligned);
        }
        if samples > MAX_CAPTURE_SAMPLES {
            return Err(Error::TooLarge);
        }
        for chunk in bytes.as_chunks::<2>().0 {
            if self.len == MAX_CAPTURE_SAMPLES {
                self.read = (self.read + 1) % MAX_CAPTURE_SAMPLES;
                self.len -= 1;
                self.dropped_samples += 1;
            }
            self.samples[self.write] = i16::from_le_bytes([chunk[0], chunk[1]]);
            self.write = (self.write + 1) % MAX_CAPTURE_SAMPLES;
            self.len += 1;
        }
        Ok(samples / usize::from(self.format.channels))
    }
    pub fn pop_s16le(&mut self, output: &mut [u8]) -> usize {
        let count = (output.len() / 2).min(self.len);
        for index in 0..count {
            output[index * 2..index * 2 + 2]
                .copy_from_slice(&self.samples[self.read].to_le_bytes());
            self.read = (self.read + 1) % MAX_CAPTURE_SAMPLES;
        }
        self.len -= count;
        count * 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixer_saturates_and_rejects_bad_frames() {
        let mut mixer = Mixer::new(PcmFormat::STEREO_48_KHZ);
        assert_eq!(mixer.mix_s16le(&[0xff, 0x7f, 0xff, 0x7f]), Ok(1));
        let mut out = [0; 4];
        assert_eq!(mixer.render_s16le(&mut out), Ok(4));
        assert_eq!(out, [0xff, 0x7f, 0xff, 0x7f]);
        assert_eq!(mixer.mix_s16le(&[1]), Err(Error::Misaligned));
    }
    #[test]
    fn capture_overflow_is_bounded_and_observable() {
        let mut ring = CaptureRing::new(PcmFormat::STEREO_48_KHZ);
        let input = [0u8; (MAX_CAPTURE_SAMPLES - 2) * 2];
        assert_eq!(ring.push_s16le(&input), Ok((MAX_CAPTURE_SAMPLES - 2) / 2));
        assert_eq!(ring.push_s16le(&[2, 0, 2, 0, 2, 0, 2, 0]), Ok(2));
        assert_eq!(ring.dropped_samples(), 2);
    }
}
