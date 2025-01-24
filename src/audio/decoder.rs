use crate::common::{Result, RhythmixError};
use rodio::{Decoder, Source};
use std::io::{Cursor, Read, Seek};

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
        // Seek to start of file
        reader.seek(std::io::SeekFrom::Start(0)).map_err(|e| {
            RhythmixError::AudioDecoding(format!("Failed to seek to start of file: {}", e))
        })?;

        // Check for minimum file size
        let file_size = reader.seek(std::io::SeekFrom::End(0)).map_err(|e| {
            RhythmixError::AudioDecoding(format!("Failed to get file size: {}", e))
        })?;

        if file_size < 128 {
            return Err(RhythmixError::AudioDecoding(
                "File too small to be valid audio".into(),
            ));
        }

        // Seek back to start
        reader.seek(std::io::SeekFrom::Start(0)).map_err(|e| {
            RhythmixError::AudioDecoding(format!("Failed to seek back to start: {}", e))
        })?;

        // Create decoder
        log::info!("Creating decoder for file of size {} bytes", file_size);
        let decoder = Decoder::new(reader).map_err(|e| {
            RhythmixError::AudioDecoding(format!("Failed to create decoder: {}", e))
        })?;

        log::info!("Decoder created successfully");
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();

        log::info!("Sample rate: {}, Channels: {}", sample_rate, channels);

        // Convert samples to f32 and collect them
        let samples: Vec<f32> = decoder.convert_samples().collect();
        log::info!("Collected {} samples", samples.len());

        if samples.is_empty() {
            return Err(RhythmixError::AudioDecoding(
                "No samples found in audio file".into(),
            ));
        }

        log::info!("Successfully decoded audio file");

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
        assert!(!decoder.to_mono().is_empty());
    }
}
