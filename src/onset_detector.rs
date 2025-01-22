use crate::error::Result;
use crate::fft_processor::FftProcessor;
use std::collections::VecDeque;

/// Configuration for onset detection
#[derive(Debug, Clone)]
pub struct OnsetConfig {
    /// Size of the FFT window
    pub window_size: usize,
    /// Size of the hop between windows
    pub hop_size: usize,
    /// Number of frequency bands to analyze
    pub num_bands: usize,
    /// Threshold for onset detection
    pub threshold: f32,
    /// Minimum time between onsets (in windows)
    pub min_interval: usize,
}

impl Default for OnsetConfig {
    fn default() -> Self {
        Self {
            window_size: 2048,
            hop_size: 512,
            num_bands: 32,
            threshold: 0.3,
            min_interval: 4,
        }
    }
}

/// Detects note onsets in audio data using spectral flux
pub struct OnsetDetector {
    config: OnsetConfig,
    fft: FftProcessor,
    prev_magnitudes: Option<Vec<f32>>,
    flux_history: VecDeque<f32>,
    last_onset: usize,
    sample_rate: u32,
}

impl OnsetDetector {
    /// Creates a new OnsetDetector with the specified configuration
    ///
    /// # Arguments
    /// * `config` - Configuration for onset detection
    /// * `sample_rate` - Sample rate of the audio
    pub fn new(config: OnsetConfig, sample_rate: u32) -> Result<Self> {
        Ok(Self {
            fft: FftProcessor::new(config.window_size)?,
            config,
            prev_magnitudes: None,
            flux_history: VecDeque::with_capacity(32),
            last_onset: 0,
            sample_rate,
        })
    }

    /// Process a window of audio samples and detect onsets
    ///
    /// # Arguments
    /// * `samples` - Audio samples to process
    ///
    /// # Returns
    /// true if an onset was detected, false otherwise
    pub fn process(&mut self, samples: &[f32]) -> Result<bool> {
        // Compute FFT magnitudes
        let mut magnitudes = self.fft.process(samples)?;

        // Compute frequency bands
        let bands = self.compute_frequency_bands(&magnitudes);

        // If this is the first frame, store it and return
        if self.prev_magnitudes.is_none() {
            self.prev_magnitudes = Some(bands);
            return Ok(false);
        }

        // Compute spectral flux
        let flux = self.compute_spectral_flux(&bands);

        // Update flux history
        self.flux_history.push_back(flux);
        if self.flux_history.len() > 32 {
            self.flux_history.pop_front();
        }

        // Check if this is an onset
        let is_onset = self.detect_onset(flux);

        // Update previous magnitudes
        self.prev_magnitudes = Some(bands);

        Ok(is_onset)
    }

    /// Compute frequency bands from FFT magnitudes
    fn compute_frequency_bands(&self, magnitudes: &[f32]) -> Vec<f32> {
        let mut bands = vec![0.0; self.config.num_bands];
        let bins_per_band = (magnitudes.len() / self.config.num_bands).max(1);

        for (i, band) in bands.iter_mut().enumerate() {
            let start = i * bins_per_band;
            let end = ((i + 1) * bins_per_band).min(magnitudes.len());
            *band = magnitudes[start..end].iter().sum::<f32>() / (end - start) as f32;
        }

        bands
    }

    /// Compute spectral flux between current and previous frame
    fn compute_spectral_flux(&self, current: &[f32]) -> f32 {
        let prev = self.prev_magnitudes.as_ref().unwrap();

        current
            .iter()
            .zip(prev.iter())
            .map(|(curr, prev)| {
                // Half-wave rectification (keep only increases in energy)
                (curr - prev).max(0.0)
            })
            .sum()
    }

    /// Detect if current frame contains an onset
    fn detect_onset(&mut self, flux: f32) -> bool {
        // Compute local average
        let local_avg = if !self.flux_history.is_empty() {
            self.flux_history.iter().sum::<f32>() / self.flux_history.len() as f32
        } else {
            flux
        };

        // Check if flux exceeds threshold and minimum interval has passed
        if flux > local_avg * (1.0 + self.config.threshold)
            && self.last_onset >= self.config.min_interval
        {
            self.last_onset = 0;
            true
        } else {
            self.last_onset += 1;
            false
        }
    }

    /// Convert window index to timestamp
    ///
    /// # Arguments
    /// * `window` - Window index
    ///
    /// # Returns
    /// Timestamp in seconds
    pub fn window_to_time(&self, window: usize) -> f64 {
        window as f64 * self.config.hop_size as f64 / self.sample_rate as f64
    }

    /// Get the minimum time between onsets
    pub fn min_onset_interval(&self) -> f64 {
        self.config.min_interval as f64 * self.config.hop_size as f64 / self.sample_rate as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn generate_test_signal(sample_rate: u32, duration: f32) -> Vec<f32> {
        let num_samples = (sample_rate as f32 * duration) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        // Generate a signal with clear onsets
        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let freq = if (t * 4.0).floor() % 2.0 == 0.0 {
                440.0
            } else {
                880.0
            };
            samples.push((2.0 * PI * freq * t).sin());
        }

        samples
    }

    #[test]
    fn test_onset_detection() {
        let sample_rate = 44100;
        let config = OnsetConfig {
            window_size: 1024,
            hop_size: 512,
            num_bands: 16,
            threshold: 0.3,
            min_interval: 4,
        };

        let mut detector = OnsetDetector::new(config, sample_rate).unwrap();
        let signal = generate_test_signal(sample_rate, 1.0);

        let mut onsets = Vec::new();
        let mut window_start = 0;

        while window_start + 1024 <= signal.len() {
            let window = &signal[window_start..window_start + 1024];
            if detector.process(window).unwrap() {
                onsets.push(detector.window_to_time(window_start / 512));
            }
            window_start += 512;
        }

        // We should detect multiple onsets in our test signal
        assert!(!onsets.is_empty());

        // Onsets should be roughly 0.25 seconds apart in our test signal
        if onsets.len() >= 2 {
            for i in 1..onsets.len() {
                let interval = onsets[i] - onsets[i - 1];
                assert!((interval - 0.25).abs() < 0.1);
            }
        }
    }

    #[test]
    fn test_frequency_bands() {
        let config = OnsetConfig::default();
        let detector = OnsetDetector::new(config.clone(), 44100).unwrap();

        let magnitudes = vec![1.0; 1024];
        let bands = detector.compute_frequency_bands(&magnitudes);

        assert_eq!(bands.len(), config.num_bands);
        assert!(bands.iter().all(|&x| x == 1.0));
    }
}
