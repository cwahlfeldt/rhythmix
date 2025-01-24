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
    /// Phase deviation threshold
    pub phase_threshold: f32,
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
            phase_threshold: 0.15,
            min_interval: 4,
        }
    }
}

/// Detects note onsets in audio data using spectral flux
pub struct OnsetDetector {
    config: OnsetConfig,
    pub fft: FftProcessor, // Made public
    prev_magnitudes: Option<Vec<f32>>,
    prev_phases: Option<Vec<f32>>,
    phase_diffs: Option<Vec<f32>>,
    flux_history: VecDeque<f32>,
    phase_dev_history: VecDeque<f32>,
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
            prev_phases: None,
            phase_diffs: None,
            flux_history: VecDeque::with_capacity(32),
            phase_dev_history: VecDeque::with_capacity(32),
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
        // Compute FFT magnitudes and phases
        let (magnitudes, phases) = self.fft.process_with_phases(samples)?;

        // Compute frequency bands
        let bands = self.compute_frequency_bands(&magnitudes);

        // If this is the first frame, store it and return
        if self.prev_magnitudes.is_none() {
            self.prev_magnitudes = Some(bands.clone());
            self.prev_phases = Some(phases.clone());
            self.phase_diffs = Some(vec![0.0; phases.len()]);
            return Ok(false);
        }

        // Update phase differences
        if let Some(prev_phases) = &self.prev_phases {
            let mut phase_diffs = Vec::with_capacity(phases.len());
            for (&curr, &prev) in phases.iter().zip(prev_phases.iter()) {
                let mut diff = curr - prev;
                while diff > std::f32::consts::PI {
                    diff -= 2.0 * std::f32::consts::PI;
                }
                while diff < -std::f32::consts::PI {
                    diff += 2.0 * std::f32::consts::PI;
                }
                phase_diffs.push(diff);
            }
            self.phase_diffs = Some(phase_diffs);
        }

        // Compute spectral flux and phase deviation
        let flux = self.compute_spectral_flux(&bands);

        // Update histories
        self.flux_history.push_back(flux);
        if self.flux_history.len() > 32 {
            self.flux_history.pop_front();
        }

        // Check if this is an onset using both spectral and phase information
        let is_onset = self.detect_onset(flux);

        // Update previous state
        self.prev_magnitudes = Some(bands);
        self.prev_phases = Some(phases);

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

    /// Detect if current frame contains an onset using both spectral flux and phase deviation
    fn detect_onset(&mut self, flux: f32) -> bool {
        // Process phase deviation if we have previous phases
        let phase_dev = if let (Some(prev_phases), Some(phase_diffs)) =
            (&self.prev_phases, &self.phase_diffs)
        {
            self.compute_phase_deviation(prev_phases, phase_diffs)
        } else {
            0.0
        };

        // Update phase deviation history
        self.phase_dev_history.push_back(phase_dev);
        if self.phase_dev_history.len() > 32 {
            self.phase_dev_history.pop_front();
        }

        // Compute local averages
        let flux_avg = if !self.flux_history.is_empty() {
            self.flux_history.iter().sum::<f32>() / self.flux_history.len() as f32
        } else {
            flux
        };

        let phase_avg = if !self.phase_dev_history.is_empty() {
            self.phase_dev_history.iter().sum::<f32>() / self.phase_dev_history.len() as f32
        } else {
            phase_dev
        };

        // Detect onset if either method indicates one and minimum interval has passed
        let flux_onset = flux > flux_avg * (1.0 + self.config.threshold);
        let phase_onset = phase_dev > phase_avg * (1.0 + self.config.phase_threshold);

        if (flux_onset || phase_onset) && self.last_onset >= self.config.min_interval {
            self.last_onset = 0;
            true
        } else {
            self.last_onset += 1;
            false
        }
    }

    /// Compute phase deviation for onset detection
    fn compute_phase_deviation(&self, prev_phases: &[f32], phase_diffs: &[f32]) -> f32 {
        let mut deviation = 0.0;
        let mut count = 0;

        for i in 1..prev_phases.len() - 1 {
            // Calculate predicted phase
            let predicted = prev_phases[i] + phase_diffs[i];

            // Calculate circular distance to actual phase
            let mut diff = (predicted - prev_phases[i]).abs();
            if diff > std::f32::consts::PI {
                diff = 2.0 * std::f32::consts::PI - diff;
            }

            // Weight by magnitude
            if let Some(magnitudes) = &self.prev_magnitudes {
                if i < magnitudes.len() {
                    diff *= magnitudes[i];
                }
            }

            deviation += diff;
            count += 1;
        }

        if count > 0 {
            deviation / count as f32
        } else {
            0.0
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
    fn test_spectral_onset_detection() {
        let sample_rate = 44100;
        let config = OnsetConfig {
            window_size: 1024,
            hop_size: 512,
            num_bands: 16,
            threshold: 0.3,
            phase_threshold: 0.0, // Disable phase detection
            min_interval: 4,
        };

        let mut detector = OnsetDetector::new(config, sample_rate).unwrap();
        let onsets = vec![(0.2, 1.0), (0.5, 1.0), (0.8, 1.0)];
        let signal = generate_test_signal(sample_rate, 1.0);

        let mut detected = Vec::new();
        let mut window_start = 0;

        while window_start + 1024 <= signal.len() {
            let window = &signal[window_start..window_start + 1024];
            if detector.process(window).unwrap() {
                detected.push(window_start as f32 / sample_rate as f32);
            }
            window_start += 512;
        }

        // Verify each expected onset was detected
        for &(expected, _) in &onsets {
            assert!(
                detected.iter().any(|&t| (t - expected).abs() < 0.05),
                "Missing onset at {}s",
                expected
            );
        }
    }

    #[test]
    fn test_phase_onset_detection() {
        let sample_rate = 44100;
        let config = OnsetConfig {
            window_size: 1024,
            hop_size: 512,
            num_bands: 16,
            threshold: 0.0, // Disable spectral detection
            phase_threshold: 0.15,
            min_interval: 4,
        };

        let mut detector = OnsetDetector::new(config, sample_rate).unwrap();
        let onsets = vec![(0.2, 1.0), (0.5, 1.0), (0.8, 1.0)];
        let signal = generate_test_signal(sample_rate, 1.0);

        let mut detected = Vec::new();
        let mut window_start = 0;

        while window_start + 1024 <= signal.len() {
            let window = &signal[window_start..window_start + 1024];
            if detector.process(window).unwrap() {
                detected.push(window_start as f32 / sample_rate as f32);
            }
            window_start += 512;
        }

        // Verify phase-based detection works
        for &(expected, _) in &onsets {
            assert!(
                detected.iter().any(|&t| (t - expected).abs() < 0.05),
                "Missing phase onset at {}s",
                expected
            );
        }
    }

    #[test]
    fn test_combined_detection() {
        let sample_rate = 44100;
        let config = OnsetConfig {
            window_size: 1024,
            hop_size: 512,
            num_bands: 16,
            threshold: 0.3,
            phase_threshold: 0.15,
            min_interval: 4,
        };

        let mut detector = OnsetDetector::new(config, sample_rate).unwrap();
        let onsets = vec![(0.2, 1.0), (0.5, 1.0), (0.8, 1.0)];
        let signal = generate_test_signal(sample_rate, 1.0);

        let mut detected = Vec::new();
        let mut window_start = 0;

        while window_start + 1024 <= signal.len() {
            let window = &signal[window_start..window_start + 1024];
            if detector.process(window).unwrap() {
                detected.push(window_start as f32 / sample_rate as f32);
            }
            window_start += 512;
        }

        // Verify combined detection
        for &(expected, _) in &onsets {
            assert!(
                detected.iter().any(|&t| (t - expected).abs() < 0.05),
                "Missing onset at {}s",
                expected
            );
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
