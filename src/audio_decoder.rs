use crate::error::{Result, RhythmixError};
use rodio::{Decoder, Source};
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::Path;

/// Handles decoding of audio files into raw samples
pub struct AudioDecoder {
    /// Sample rate of the decoded audio
    sample_rate: u32,
    /// Number of channels in the audio
    channels: u16,
    /// Raw audio samples
    samples: Vec<f32>,
}

impl AudioDecoder {
    /// Creates a new AudioDecoder from a file path
    ///
    /// # Arguments
    /// * `path` - Path to the audio file
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or decoded
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path).map_err(|e| {
            RhythmixError::AudioDecoding(format!("Failed to open audio file: {}", e))
        })?;

        Self::from_reader(BufReader::new(file))
    }

    /// Creates a new AudioDecoder from bytes
    ///
    /// # Arguments
    /// * `bytes` - Raw bytes of the audio file
    ///
    /// # Errors
    /// Returns an error if the bytes cannot be decoded
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        Self::from_reader(Cursor::new(bytes))
    }

    /// Creates a new AudioDecoder from a reader
    ///
    /// # Arguments
    /// * `reader` - Any type that implements Read + Seek
    ///
    /// # Errors
    /// Returns an error if the audio cannot be decoded
    fn from_reader<R>(mut reader: R) -> Result<Self>
    where
        R: Read + Seek + Send + Sync + 'static,
    {
        let decoder = Decoder::new(reader).map_err(|e| {
            RhythmixError::AudioDecoding(format!("Failed to create decoder: {}", e))
        })?;

        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();

        // Convert samples to f32 and collect them
        let samples: Vec<f32> = decoder.convert_samples().collect();

        if samples.is_empty() {
            return Err(RhythmixError::AudioDecoding(
                "No samples found in audio file".into(),
            ));
        }

        Ok(Self {
            sample_rate,
            channels,
            samples,
        })
    }

    /// Gets the sample rate of the audio
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Gets the number of channels in the audio
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Gets the duration of the audio in seconds
    pub fn duration(&self) -> f64 {
        self.samples.len() as f64 / (self.sample_rate as f64 * self.channels as f64)
    }

    /// Gets a reference to the raw samples
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Converts stereo to mono by averaging channels if necessary
    ///
    /// # Returns
    /// A new vector containing mono samples
    pub fn to_mono(&self) -> Vec<f32> {
        if self.channels == 1 {
            self.samples.clone()
        } else {
            // Average all channels to create mono
            let samples_per_channel = self.samples.len() / self.channels as usize;
            let mut mono_samples = Vec::with_capacity(samples_per_channel);

            for frame in 0..samples_per_channel {
                let mut sum = 0.0;
                for channel in 0..self.channels as usize {
                    sum += self.samples[frame * self.channels as usize + channel];
                }
                mono_samples.push(sum / self.channels as f32);
            }

            mono_samples
        }
    }

    /// Gets a chunk of samples starting at the specified offset
    ///
    /// # Arguments
    /// * `offset` - Starting sample index
    /// * `size` - Number of samples to get
    ///
    /// # Returns
    /// A vector containing the requested samples, or None if out of bounds
    pub fn get_chunk(&self, offset: usize, size: usize) -> Option<Vec<f32>> {
        if offset + size <= self.samples.len() {
            Some(self.samples[offset..offset + size].to_vec())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn create_test_wav() -> Vec<u8> {
        // Create a simple sine wave
        let sample_rate = 44100u32;
        let duration = 1.0; // seconds
        let frequency = 440.0; // Hz
        let num_samples = (sample_rate as f32 * duration) as usize;

        let samples: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * PI * frequency * i as f32 / sample_rate as f32).sin())
            .collect();

        // Convert to bytes (this is a simplified WAV format)
        let mut wav = Vec::new();

        // Calculate sizes
        let data_size = (samples.len() * 4) as u32;
        let file_size = 36u32 + data_size;

        // RIFF header
        wav.extend(b"RIFF");
        wav.extend(&file_size.to_le_bytes());
        wav.extend(b"WAVE");

        // Format chunk
        wav.extend(b"fmt ");
        wav.extend(&16u32.to_le_bytes()); // Subchunk1Size
        wav.extend(&3u16.to_le_bytes()); // AudioFormat (3 = IEEE float)
        wav.extend(&1u16.to_le_bytes()); // NumChannels
        wav.extend(&sample_rate.to_le_bytes()); // SampleRate
        wav.extend(&(sample_rate * 4).to_le_bytes()); // ByteRate
        wav.extend(&4u16.to_le_bytes()); // BlockAlign
        wav.extend(&32u16.to_le_bytes()); // BitsPerSample

        // Data chunk
        wav.extend(b"data");
        wav.extend(&data_size.to_le_bytes());

        // Sample data
        for sample in samples {
            wav.extend(&sample.to_le_bytes());
        }

        wav
    }

    #[test]
    fn test_decode_wav() {
        let wav_data = create_test_wav();
        let decoder = AudioDecoder::from_bytes(wav_data).unwrap();

        assert_eq!(decoder.sample_rate(), 44100);
        assert_eq!(decoder.channels(), 1);
        assert!((decoder.duration() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_to_mono() {
        let wav_data = create_test_wav();
        let decoder = AudioDecoder::from_bytes(wav_data).unwrap();
        let mono = decoder.to_mono();

        assert_eq!(mono.len(), decoder.samples().len());
    }

    #[test]
    fn test_get_chunk() {
        let wav_data = create_test_wav();
        let decoder = AudioDecoder::from_bytes(wav_data).unwrap();

        let chunk = decoder.get_chunk(0, 1000).unwrap();
        assert_eq!(chunk.len(), 1000);

        assert!(decoder.get_chunk(decoder.samples().len(), 1000).is_none());
    }
}
