use crate::common::Result;
use super::spectral::SpectralAnalyzer;

/// Audio feature extraction configuration
#[derive(Debug, Clone)]
pub struct FeatureConfig {
    /// Size of the FFT window
    pub window_size: usize,
    /// Number of frequency bands for spectral features
    pub num_bands: usize,
    /// Minimum frequency for bass band (Hz)
    pub bass_band_min: f32,
    /// Maximum frequency for bass band (Hz)
    pub bass_band_max: f32,
    /// Minimum frequency for mid band (Hz)
    pub mid_band_min: f32,
    /// Maximum frequency for mid band (Hz)
    pub mid_band_max: f32,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            window_size: 2048,
            num_bands: 32,
            bass_band_min: 20.0,
            bass_band_max: 250.0,
            mid_band_min: 250.0,
            mid_band_max: 4000.0,
        }
    }
}

/// Extracted audio features
#[derive(Debug, Clone)]
pub struct AudioFeatures {
    /// RMS energy of the signal
    pub rms: f32,
    /// Spectral centroid (brightness)
    pub centroid: f32,
    /// Spectral spread
    pub spread: f32,
    /// Bass band energy
    pub bass_energy: f32,
    /// Mid band energy
    pub mid_energy: f32,
    /// High band energy
    pub high_energy: f32,
    /// Spectral rolloff frequency
    pub rolloff: f32,
    /// Zero crossing rate
    pub zero_crossing_rate: f32,
}

/// Extracts audio features from the signal
pub struct FeatureExtractor {
    config: FeatureConfig,
    spectral: SpectralAnalyzer,
    sample_rate: u32,
}

impl FeatureExtractor {
    /// Creates a new FeatureExtractor with the specified configuration
    pub fn new(config: FeatureConfig, sample_rate: u32) -> Result<Self> {
        Ok(Self {
            spectral: SpectralAnalyzer::new(config.window_size)?,
            config,
            sample_rate,
        })
    }

    /// Process a window of audio samples and extract features
    pub fn process(&mut self, samples: &[f32]) -> Result<AudioFeatures> {
        // Calculate RMS energy
        let rms = (samples.iter().map(|&x| x * x).sum::<f32>() / samples.len() as f32).sqrt();

        // Calculate zero crossing rate
        let zcr = samples.windows(2).fold(0.0, |acc, window| {
            if window[0].signum() != window[1].signum() {
                acc + 1.0
            } else {
                acc
            }
        }) / samples.len() as f32;

        // Compute spectral features
        let (magnitudes, _) = self.spectral.process_with_phases(samples)?;

        // Calculate spectral centroid and spread
        let (centroid, spread) = self.compute_spectral_moments(&magnitudes);

        // Calculate band energies
        let (bass, mid, high) = self.compute_band_energies(&magnitudes);

        // Calculate spectral rolloff
        let rolloff = self.compute_spectral_rolloff(&magnitudes);

        Ok(AudioFeatures {
            rms,
            centroid,
            spread,
            bass_energy: bass,
            mid_energy: mid,
            high_energy: high,
            rolloff,
            zero_crossing_rate: zcr,
        })
    }

    /// Compute spectral centroid and spread
    fn compute_spectral_moments(&self, magnitudes: &[f32]) -> (f32, f32) {
        let total_energy: f32 = magnitudes.iter().sum();
        if total_energy == 0.0 {
            return (0.0, 0.0);
        }

        // Calculate centroid
        let centroid: f32 = magnitudes
            .iter()
            .enumerate()
            .map(|(i, &m)| {
                let freq = self.spectral.bin_to_frequency(i, self.sample_rate);
                freq * m
            })
            .sum::<f32>() / total_energy;

        // Calculate spread
        let spread: f32 = magnitudes
            .iter()
            .enumerate()
            .map(|(i, &m)| {
                let freq = self.spectral.bin_to_frequency(i, self.sample_rate);
                (freq - centroid).powi(2) * m
            })
            .sum::<f32>() / total_energy;

        (centroid, spread.sqrt())
    }

    /// Compute energies in different frequency bands
    fn compute_band_energies(&self, magnitudes: &[f32]) -> (f32, f32, f32) {
        let mut bass_energy = 0.0;
        let mut mid_energy = 0.0;
        let mut high_energy = 0.0;

        for (i, &magnitude) in magnitudes.iter().enumerate() {
            let freq = self.spectral.bin_to_frequency(i, self.sample_rate);
            
            if freq >= self.config.bass_band_min && freq < self.config.bass_band_max {
                bass_energy += magnitude * magnitude;
            } else if freq >= self.config.mid_band_min && freq < self.config.mid_band_max {
                mid_energy += magnitude * magnitude;
            } else if freq >= self.config.mid_band_max {
                high_energy += magnitude * magnitude;
            }
        }

        // Normalize energies
        let total_energy = bass_energy + mid_energy + high_energy;
        if total_energy > 0.0 {
            bass_energy /= total_energy;
            mid_energy /= total_energy;
            high_energy /= total_energy;
        }

        (bass_energy, mid_energy, high_energy)
    }

    /// Compute spectral rolloff frequency (frequency below which 85% of energy exists)
    fn compute_spectral_rolloff(&self, magnitudes: &[f32]) -> f32 {
        let total_energy: f32 = magnitudes.iter().map(|&m| m * m).sum();
        let threshold = total_energy * 0.85;
        let mut cumulative_energy = 0.0;

        for (i, &magnitude) in magnitudes.iter().enumerate() {
            cumulative_energy += magnitude * magnitude;
            if cumulative_energy >= threshold {
                return self.spectral.bin_to_frequency(i, self.sample_rate);
            }
        }

        self.spectral.bin_to_frequency(magnitudes.len() - 1, self.sample_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn generate_test_signals() -> Vec<Vec<f32>> {
        let sample_rate = 44100;
        let duration = 0.1; // 100ms
        let num_samples = (sample_rate as f32 * duration) as usize;
        
        vec![
            // Low frequency sine wave (100 Hz)
            (0..num_samples)
                .map(|i| (2.0 * PI * 100.0 * i as f32 / sample_rate as f32).sin())
                .collect(),
            
            // High frequency sine wave (2000 Hz)
            (0..num_samples)
                .map(|i| (2.0 * PI * 2000.0 * i as f32 / sample_rate as f32).sin())
                .collect(),
            
            // Mix of frequencies
            (0..num_samples)
                .map(|i| {
                    let t = i as f32 / sample_rate as f32;
                    0.5 * (2.0 * PI * 100.0 * t).sin() + 
                    0.3 * (2.0 * PI * 1000.0 * t).sin() +
                    0.2 * (2.0 * PI * 3000.0 * t).sin()
                })
                .collect(),
        ]
    }

    #[test]
    fn test_feature_extraction() {
        let config = FeatureConfig::default();
        let mut extractor = FeatureExtractor::new(config, 44100).unwrap();
        let signals = generate_test_signals();

        // Test low frequency signal
        let features = extractor.process(&signals[0]).unwrap();
        assert!(features.bass_energy > features.high_energy);
        assert!(features.centroid < 500.0);

        // Test high frequency signal
        let features = extractor.process(&signals[1]).unwrap();
        assert!(features.high_energy > features.bass_energy);
        assert!(features.centroid > 1000.0);

        // Test mixed signal
        let features = extractor.process(&signals[2]).unwrap();
        assert!(features.mid_energy > 0.1);
        assert!(features.rms > 0.0 && features.rms < 1.0);
        assert!(features.rolloff > 0.0);
    }
}
