use crate::common::{Result, RhythmixError};
use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::Arc;

/// Handles spectral analysis for audio processing
pub struct SpectralAnalyzer {
    fft: Arc<dyn rustfft::Fft<f32>>,
    window_size: usize,
    window: Vec<f32>,
}

impl SpectralAnalyzer {
    /// Creates a new spectral analyzer with the specified window size
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

    /// Process a chunk of audio samples using FFT, returning magnitudes and phases
    ///
    /// # Arguments
    /// * `samples` - Audio samples to process
    ///
    /// # Returns
    /// Tuple of (magnitudes, phases) vectors
    ///
    /// # Errors
    /// Returns an error if the input length doesn't match the window size
    pub fn process_with_phases(&self, samples: &[f32]) -> Result<(Vec<f32>, Vec<f32>)> {
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

        // Calculate magnitudes and phases (only up to Nyquist frequency)
        let nyquist_buffer = &buffer[..=self.window_size / 2];
        let mut magnitudes = Vec::with_capacity(nyquist_buffer.len());
        let mut phases = Vec::with_capacity(nyquist_buffer.len());

        for c in nyquist_buffer {
            magnitudes.push((c.norm() / self.window_size as f32).sqrt());
            phases.push(c.im.atan2(c.re));
        }

        Ok((magnitudes, phases))
    }

    /// Compute energy bands from FFT magnitudes
    ///
    /// # Arguments
    /// * `magnitudes` - FFT magnitudes
    /// * `num_bands` - Number of frequency bands to compute
    pub fn compute_frequency_bands(&self, magnitudes: &[f32], num_bands: usize) -> Vec<f32> {
        let mut bands = vec![0.0; num_bands];
        let bins_per_band = (magnitudes.len() / num_bands).max(1);

        for (i, band) in bands.iter_mut().enumerate() {
            let start = i * bins_per_band;
            let end = ((i + 1) * bins_per_band).min(magnitudes.len());
            *band = magnitudes[start..end].iter().sum::<f32>() / (end - start) as f32;
        }

        bands
    }

    /// Calculate frequency resolution for a given sample rate
    ///
    /// # Arguments
    /// * `sample_rate` - Audio sample rate in Hz
    pub fn frequency_resolution(&self, sample_rate: u32) -> f32 {
        sample_rate as f32 / self.window_size as f32
    }

    /// Convert frequency bin index to frequency in Hz
    ///
    /// # Arguments
    /// * `bin` - Frequency bin index
    /// * `sample_rate` - Audio sample rate in Hz
    pub fn bin_to_frequency(&self, bin: usize, sample_rate: u32) -> f32 {
        bin as f32 * self.frequency_resolution(sample_rate)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_spectral_analyzer_creation() {
        assert!(SpectralAnalyzer::new(1024).is_ok());
        assert!(SpectralAnalyzer::new(1000).is_err()); // Not power of 2
    }

    #[test]
    fn test_process_sine_wave() {
        let window_size = 1024;
        let analyzer = SpectralAnalyzer::new(window_size).unwrap();
        let sample_rate = 44100;
        let frequency = 440.0; // A4 note

        // Generate sine wave
        let samples: Vec<f32> = (0..window_size)
            .map(|i| (2.0 * PI * frequency * i as f32 / sample_rate as f32).sin())
            .collect();

        let (magnitudes, _) = analyzer.process_with_phases(&samples).unwrap();

        // Find peak frequency bin
        let peak_bin = magnitudes
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        // Calculate expected frequency bin
        let expected_bin =
            (frequency / analyzer.frequency_resolution(sample_rate)).round() as usize;

        // Allow for some margin due to windowing
        assert!((peak_bin as i32 - expected_bin as i32).abs() <= 1);
    }

    #[test]
    fn test_frequency_bands() {
        let analyzer = SpectralAnalyzer::new(1024).unwrap();
        let magnitudes = vec![1.0; 513]; // Nyquist length
        let bands = analyzer.compute_frequency_bands(&magnitudes, 8);

        assert_eq!(bands.len(), 8);
        assert!(bands.iter().all(|&x| x > 0.0));
    }
}
