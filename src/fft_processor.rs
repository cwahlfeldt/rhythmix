use crate::error::{Result, RhythmixError};
use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::Arc;

/// Handles FFT processing for audio analysis
pub struct FftProcessor {
    fft: Arc<dyn rustfft::Fft<f32>>,
    window_size: usize,
    window: Vec<f32>,
}

impl FftProcessor {
    /// Creates a new FFT processor with the specified window size
    ///
    /// # Arguments
    /// * `window_size` - Size of the FFT window (must be a power of 2)
    ///
    /// # Errors
    /// Returns an error if the window size is not a power of 2
    pub fn new(window_size: usize) -> Result<Self> {
        if !window_size.is_power_of_two() {
            return Err(RhythmixError::FftProcessing(
                "FFT window size must be a power of 2".into(),
            ));
        }

        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(window_size);

        // Create Hann window for better frequency resolution
        let window = Self::create_hann_window(window_size);

        Ok(Self {
            fft,
            window_size,
            window,
        })
    }

    /// Process a chunk of audio samples using FFT
    ///
    /// # Arguments
    /// * `samples` - Audio samples to process
    ///
    /// # Returns
    /// Vector of frequency magnitudes
    ///
    /// # Errors
    /// Returns an error if the input length doesn't match the window size
    pub fn process(&self, samples: &[f32]) -> Result<Vec<f32>> {
        if samples.len() != self.window_size {
            return Err(RhythmixError::FftProcessing(format!(
                "Input length {} does not match window size {}",
                samples.len(),
                self.window_size
            )));
        }

        // Apply window function and convert to complex numbers
        let mut buffer: Vec<Complex<f32>> = samples
            .iter()
            .zip(self.window.iter())
            .map(|(&s, &w)| Complex::new(s * w, 0.0))
            .collect();

        // Perform FFT
        self.fft.process(&mut buffer);

        // Calculate magnitudes (only up to Nyquist frequency)
        let magnitudes: Vec<f32> = buffer[..=self.window_size / 2]
            .iter()
            .map(|c| (c.norm() / self.window_size as f32).sqrt())
            .collect();

        Ok(magnitudes)
    }

    /// Get the frequency resolution of the FFT
    ///
    /// # Arguments
    /// * `sample_rate` - Sample rate of the audio in Hz
    ///
    /// # Returns
    /// Frequency resolution in Hz per bin
    pub fn frequency_resolution(&self, sample_rate: u32) -> f32 {
        sample_rate as f32 / self.window_size as f32
    }

    /// Creates a Hann window for the given size
    fn create_hann_window(size: usize) -> Vec<f32> {
        (0..size)
            .map(|i| {
                let x = 2.0 * std::f32::consts::PI * i as f32 / (size - 1) as f32;
                0.5 * (1.0 - x.cos())
            })
            .collect()
    }

    /// Convert FFT bin index to frequency
    ///
    /// # Arguments
    /// * `bin` - FFT bin index
    /// * `sample_rate` - Sample rate of the audio in Hz
    ///
    /// # Returns
    /// Frequency in Hz
    pub fn bin_to_frequency(&self, bin: usize, sample_rate: u32) -> f32 {
        bin as f32 * self.frequency_resolution(sample_rate)
    }

    /// Find peaks in the frequency spectrum
    ///
    /// # Arguments
    /// * `magnitudes` - Vector of frequency magnitudes
    /// * `threshold` - Minimum magnitude for a peak
    ///
    /// # Returns
    /// Vector of peak indices
    pub fn find_peaks(&self, magnitudes: &[f32], threshold: f32) -> Vec<usize> {
        let mut peaks = Vec::new();

        // Skip first and last bins
        for i in 1..magnitudes.len() - 1 {
            if magnitudes[i] > threshold
                && magnitudes[i] > magnitudes[i - 1]
                && magnitudes[i] > magnitudes[i + 1]
            {
                peaks.push(i);
            }
        }

        peaks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_fft_processor_creation() {
        assert!(FftProcessor::new(1024).is_ok());
        assert!(FftProcessor::new(1000).is_err()); // Not power of 2
    }

    #[test]
    fn test_process_sine_wave() {
        let window_size = 1024;
        let processor = FftProcessor::new(window_size).unwrap();
        let sample_rate = 44100;
        let frequency = 440.0; // A4 note

        // Generate sine wave
        let samples: Vec<f32> = (0..window_size)
            .map(|i| (2.0 * PI * frequency * i as f32 / sample_rate as f32).sin())
            .collect();

        let magnitudes = processor.process(&samples).unwrap();

        // Find the frequency bin corresponding to 440 Hz
        let target_bin = (frequency / processor.frequency_resolution(sample_rate)).round() as usize;

        // Check if there's a peak at the target frequency
        let peaks = processor.find_peaks(&magnitudes, 0.1);
        assert!(peaks.contains(&target_bin));
    }

    #[test]
    fn test_frequency_resolution() {
        let processor = FftProcessor::new(1024).unwrap();
        let sample_rate = 44100;
        let expected_resolution = 44100.0 / 1024.0;

        assert!((processor.frequency_resolution(sample_rate) - expected_resolution).abs() < 0.001);
    }

    #[test]
    fn test_bin_to_frequency() {
        let processor = FftProcessor::new(1024).unwrap();
        let sample_rate = 44100;
        let bin = 10;

        let expected_freq = 10.0 * sample_rate as f32 / 1024.0;
        assert!((processor.bin_to_frequency(bin, sample_rate) - expected_freq).abs() < 0.001);
    }
}
