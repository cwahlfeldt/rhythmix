use super::spectral::SpectralAnalyzer;
use crate::common::Result;
use std::collections::VecDeque;

/// Configuration for onset detection
#[derive(Debug, Clone)]
pub struct OnsetConfig {
    /// Size of the FFT window
    pub window_size: usize,
    /// Number of frequency bands to analyze
    pub num_bands: usize,
    /// Spectral flux threshold for onset detection
    pub flux_threshold: f32,
    /// Phase deviation threshold
    pub phase_threshold: f32,
    /// Minimum time between onsets (in windows)
    pub min_interval: usize,
    /// Size of the moving average window
    pub moving_avg_size: usize,
}

impl Default for OnsetConfig {
    fn default() -> Self {
        Self {
            window_size: 1024,  // ~23ms at 44.1kHz - good for transients
            num_bands: 64,      // More bands for better frequency resolution
            flux_threshold: 0.2, // More sensitive to energy changes
            phase_threshold: 0.25, // More sensitive to phase changes
            min_interval: 2,     // Allow closer onsets (~46ms at 44.1kHz)
            moving_avg_size: 16, // Longer history for better adaptive thresholding
        }
    }
}

impl OnsetConfig {
    pub fn for_dnb() -> Self {
        Self {
            window_size: 1024,
            num_bands: 64,
            flux_threshold: 0.15,  // Even more sensitive for fast transients
            phase_threshold: 0.3,  // Higher phase sensitivity for hi-hats
            min_interval: 2,       // Allow fast patterns
            moving_avg_size: 16,
        }
    }
}

/// Onset detection result
#[derive(Debug, Clone)]
pub struct OnsetResult {
    /// Whether an onset was detected
    pub is_onset: bool,
    /// Spectral flux value
    /// Spectral flux value - used for future onset classification
    #[allow(dead_code)]
    pub flux: f32,
    /// Phase deviation value - used for future onset classification
    #[allow(dead_code)]
    pub phase_dev: f32,
    /// Overall onset strength (0.0 - 1.0)
    pub strength: f32,
}

/// Detects note onsets in audio data using spectral flux and phase deviation
pub struct OnsetDetector {
    config: OnsetConfig,
    spectral: SpectralAnalyzer,
    prev_magnitudes: Option<Vec<f32>>,
    prev_phases: Option<Vec<f32>>,
    phase_diffs: Option<Vec<f32>>,
    flux_history: VecDeque<f32>,
    phase_dev_history: VecDeque<f32>,
    last_onset: usize,
}

impl OnsetDetector {
    /// Creates a new OnsetDetector with the specified configuration
    pub fn new(config: OnsetConfig) -> Result<Self> {
        Ok(Self {
            spectral: SpectralAnalyzer::new(config.window_size)?,
            config,
            prev_magnitudes: None,
            prev_phases: None,
            phase_diffs: None,
            flux_history: VecDeque::with_capacity(32),
            phase_dev_history: VecDeque::with_capacity(32),
            last_onset: 0,
        })
    }

    /// Process a window of audio samples and detect onsets
    pub fn process(&mut self, samples: &[f32]) -> Result<OnsetResult> {
        // Compute FFT magnitudes and phases
        let (magnitudes, phases) = self.spectral.process_with_phases(samples)?;

        // Compute frequency bands
        let bands = self
            .spectral
            .compute_frequency_bands(&magnitudes, self.config.num_bands);

        // Initialize state if this is the first frame
        if self.prev_magnitudes.is_none() {
            self.prev_magnitudes = Some(bands.clone());
            self.prev_phases = Some(phases.clone());
            self.phase_diffs = Some(vec![0.0; phases.len()]);
            return Ok(OnsetResult {
                is_onset: false,
                flux: 0.0,
                phase_dev: 0.0,
                strength: 0.0,
            });
        }

        // Update phase differences
        let phase_dev = self.update_phase_diffs(&phases);

        // Compute spectral flux
        let flux = self.compute_spectral_flux(&bands);

        // Update histories
        self.update_histories(flux, phase_dev);

        // Detect onset using both methods
        let result = self.detect_onset(flux, phase_dev);

        // Update previous state
        self.prev_magnitudes = Some(bands);
        self.prev_phases = Some(phases);

        Ok(result)
    }

    /// Update phase difference calculations
    fn update_phase_diffs(&mut self, phases: &[f32]) -> f32 {
        let mut phase_dev = 0.0;

        if let (Some(prev_phases), Some(phase_diffs)) = (&self.prev_phases, &mut self.phase_diffs) {
            let mut total_dev = 0.0;
            let mut count = 0;

            for (i, (&curr, &prev)) in phases.iter().zip(prev_phases.iter()).enumerate() {
                // Calculate phase difference
                let mut diff = curr - prev;
                while diff > std::f32::consts::PI {
                    diff -= 2.0 * std::f32::consts::PI;
                }
                while diff < -std::f32::consts::PI {
                    diff += 2.0 * std::f32::consts::PI;
                }
                phase_diffs[i] = diff;

                // Calculate deviation from predicted phase
                if i > 0 && i < phases.len() - 1 {
                    let predicted = prev + diff;
                    let mut dev = (predicted - curr).abs();
                    if dev > std::f32::consts::PI {
                        dev = 2.0 * std::f32::consts::PI - dev;
                    }

                    total_dev += dev;
                    count += 1;
                }
            }

            if count > 0 {
                phase_dev = total_dev / count as f32;
            }
        }

        phase_dev
    }

    /// Compute spectral flux between current and previous frame
    fn compute_spectral_flux(&self, current: &[f32]) -> f32 {
        if let Some(prev) = &self.prev_magnitudes {
            current
                .iter()
                .zip(prev.iter())
                .map(|(curr, prev)| (curr - prev).max(0.0))
                .sum()
        } else {
            0.0
        }
    }

    /// Update flux and phase deviation histories
    fn update_histories(&mut self, flux: f32, phase_dev: f32) {
        // Update flux history
        self.flux_history.push_back(flux);
        if self.flux_history.len() > self.config.moving_avg_size {
            self.flux_history.pop_front();
        }

        // Update phase deviation history
        self.phase_dev_history.push_back(phase_dev);
        if self.phase_dev_history.len() > self.config.moving_avg_size {
            self.phase_dev_history.pop_front();
        }
    }

    /// Detect if current frame contains an onset
    fn detect_onset(&mut self, flux: f32, phase_dev: f32) -> OnsetResult {
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

        // Detect onset using both methods
        let flux_onset = flux > flux_avg * (1.0 + self.config.flux_threshold);
        let phase_onset = phase_dev > phase_avg * (1.0 + self.config.phase_threshold);

        // Calculate onset strength
        let flux_strength = if flux_avg > 0.0 {
            (flux / flux_avg - 1.0).min(1.0)
        } else {
            0.0
        };

        let phase_strength = if phase_avg > 0.0 {
            (phase_dev / phase_avg - 1.0).min(1.0)
        } else {
            0.0
        };

        // Calculate combined strength with weighted importance
        let combined_strength = 0.7 * flux_strength + 0.3 * phase_strength;

        // Adaptive minimum interval based on strength and tempo range
        let min_interval = if combined_strength > 0.8 {
            // For strong beats, allow closer spacing (good for DnB kicks/snares)
            (self.config.min_interval as f32 * 0.75) as usize
        } else if combined_strength > 0.6 {
            // Moderately strong beats
            (self.config.min_interval as f32 * 0.85) as usize
        } else {
            self.config.min_interval
        };

        // Combine methods for final decision with strength thresholds
        let is_onset = if self.last_onset >= min_interval {
            // Strong onsets need less confirmation
            if (flux_onset && flux_strength > 0.4) || 
               (phase_onset && phase_strength > 0.4) ||
               (flux_onset && phase_onset && (flux_strength + phase_strength) > 0.6) {
                self.last_onset = 0;
                true
            } else {
                false
            }
        } else {
            self.last_onset += 1;
            false
        };

        OnsetResult {
            is_onset,
            flux,
            phase_dev,
            strength: combined_strength,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn generate_test_signal(duration_samples: usize) -> Vec<f32> {
        let mut samples = Vec::with_capacity(duration_samples);

        // Generate a signal with clear onsets
        for i in 0..duration_samples {
            let t = i as f32 / 44100.0;
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
        let config = OnsetConfig {
            window_size: 1024,
            num_bands: 16,
            flux_threshold: 0.3,
            phase_threshold: 0.15,
            min_interval: 4,
            moving_avg_size: 8,
        };

        let mut detector = OnsetDetector::new(config).unwrap();
        let signal = generate_test_signal(44100); // 1 second
        let mut onsets = Vec::new();
        let mut window_start = 0;

        while window_start + 1024 <= signal.len() {
            let window = &signal[window_start..window_start + 1024];
            let result = detector.process(window).unwrap();
            if result.is_onset {
                onsets.push(window_start as f32 / 44100.0);
            }
            window_start += 512; // 50% overlap
        }

        assert!(!onsets.is_empty());

        // Check that onsets are reasonably spaced
        if onsets.len() >= 2 {
            for window in onsets.windows(2) {
                let spacing = window[1] - window[0];
                assert!(spacing >= 0.1); // At least 100ms between onsets
            }
        }
    }
}
