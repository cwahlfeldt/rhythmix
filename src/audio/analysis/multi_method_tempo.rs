use crate::common::Result;
use rustfft::num_complex::Complex;
use std::collections::VecDeque;
use super::comb_filter::{CombFilterAnalyzer, CombFilterConfig};
use super::tempo::{TempoConfig, TempoResults};

/// Configuration for multi-method tempo analysis
#[derive(Debug, Clone)]
pub struct MultiMethodTempoConfig {
    /// Window size for analysis in seconds
    pub window_size: f64,
    /// Minimum confidence required for tempo detection
    pub min_confidence: f64,
    /// Frequency bands for sub-band analysis
    pub frequency_bands: Vec<(f64, f64)>,
    /// Maximum allowed deviation between methods (in BPM)
    pub max_method_deviation: f64,
}

impl Default for MultiMethodTempoConfig {
    fn default() -> Self {
        Self {
            window_size: 5.0,
            min_confidence: 0.6,
            // Frequency bands in Hz: (low, high)
            frequency_bands: vec![
                (20.0, 200.0),   // Sub-bass & bass
                (200.0, 2000.0), // Mids
                (2000.0, 20000.0), // Highs
            ],
            max_method_deviation: 3.0,
        }
    }
}

impl MultiMethodTempoConfig {
    pub fn get_window_samples(&self, sample_rate: u32) -> usize {
        (self.window_size * sample_rate as f64) as usize
    }
}

pub struct MultiMethodTempoAnalyzer {
    config: MultiMethodTempoConfig,
    sample_rate: u32,
    comb_analyzer: CombFilterAnalyzer,
    statistical_config: TempoConfig,
    band_energies: Vec<VecDeque<f32>>,
    prev_bpm_estimate: Option<f64>,
    confidence_history: VecDeque<f64>,
}

impl MultiMethodTempoAnalyzer {
    pub fn new(config: MultiMethodTempoConfig, sample_rate: u32) -> Result<Self> {
        let comb_config = CombFilterConfig {
            min_bpm: 60.0,
            max_bpm: 200.0,
            bpm_resolution: 0.25,
            window_size: config.window_size,
        };

        let statistical_config = TempoConfig {
            min_bpm: 60.0,
            max_bpm: 200.0,
            tempo_window: config.window_size,
            confidence_threshold: config.min_confidence,
        };

        let comb_analyzer = CombFilterAnalyzer::new(comb_config, sample_rate)?;

        // Get number of bands before moving config
        let num_bands = config.frequency_bands.len();
        
        Ok(Self {
            config: config.clone(),
            sample_rate,
            comb_analyzer,
            statistical_config,
            band_energies: vec![VecDeque::new(); num_bands],
            prev_bpm_estimate: None,
            confidence_history: VecDeque::with_capacity(32),
        })
    }

    /// Analyze tempo using multiple methods and cross-validate results
    pub fn analyze_tempo(&mut self, samples: &[f32]) -> Result<TempoResults> {
        // Calculate FFT size based on the comb analyzer's window size
        let fft_size = (self.sample_rate as f64 * self.config.window_size) as usize;
        
        // Prepare analysis buffer with proper length
        let mut analysis_buffer = samples.to_vec();
        if analysis_buffer.len() > fft_size {
            analysis_buffer.truncate(fft_size);
        } else if analysis_buffer.len() < fft_size {
            analysis_buffer.resize(fft_size, 0.0);
        }

        // 1. Perform comb filter analysis
        let comb_results = self.comb_analyzer.analyze(&analysis_buffer)?;
        let mut comb_candidates: Vec<(f64, f64)> = comb_results.into_iter()
            .filter(|(_, strength)| *strength > self.config.min_confidence)
            .collect();
        comb_candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // 2. Perform sub-band analysis
        let band_results = self.analyze_frequency_bands(samples)?;
        
        // 3. Cross-validate results
        let validated_bpm = self.cross_validate_tempo(
            comb_candidates.first().map(|&(bpm, _)| bpm),
            band_results,
        );

        // 4. Calculate final confidence
        let confidence = self.calculate_final_confidence(&comb_candidates, validated_bpm);
        
        // 5. Update history
        self.prev_bpm_estimate = Some(validated_bpm);
        self.confidence_history.push_back(confidence);
        if self.confidence_history.len() > 32 {
            self.confidence_history.pop_front();
        }

        Ok(TempoResults {
            bpm: validated_bpm,
            confidence,
            phase: 0.0, // Phase calculation needs onset information
            time_signature: Default::default(), // Time signature detection needs separate analysis
        })
    }

    /// Analyze tempo in different frequency bands
    fn analyze_frequency_bands(&mut self, samples: &[f32]) -> Result<Vec<f64>> {
        let mut band_bpms = Vec::new();
        let fft_size = samples.len().next_power_of_two();
        
        // Create FFT planner
        let mut planner = rustfft::FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);

        // Prepare FFT buffer
        let mut fft_buffer: Vec<Complex<f32>> = samples.iter()
            .map(|&x| Complex::new(x, 0.0))
            .chain(std::iter::repeat(Complex::new(0.0, 0.0)))
            .take(fft_size)
            .collect();

        // Perform FFT
        fft.process(&mut fft_buffer);

        // Analyze each frequency band
        for (band_idx, (low_freq, high_freq)) in self.config.frequency_bands.iter().enumerate() {
            let low_bin = (low_freq * fft_size as f64 / self.sample_rate as f64) as usize;
            let high_bin = (high_freq * fft_size as f64 / self.sample_rate as f64) as usize;

            // Calculate band energy
            let mut band_energy = 0.0;
            for bin in low_bin..=high_bin.min(fft_size/2) {
                band_energy += fft_buffer[bin].norm() as f32;
            }
            
            // Store band energy
            self.band_energies[band_idx].push_back(band_energy);
            if self.band_energies[band_idx].len() > 128 {
                self.band_energies[band_idx].pop_front();
            }

            // Detect periodicity in band energy
            if self.band_energies[band_idx].len() >= 64 {
                let energies: Vec<f32> = self.band_energies[band_idx].iter().copied().collect();
                if let Some(period) = self.detect_periodicity(&energies) {
                    let bpm = 60.0 * self.sample_rate as f64 / period as f64;
                    if (60.0..=200.0).contains(&bpm) {
                        band_bpms.push(bpm);
                    }
                }
            }
        }

        Ok(band_bpms)
    }

    /// Detect periodicity in a signal using autocorrelation
    fn detect_periodicity(&self, signal: &[f32]) -> Option<usize> {
        let len = signal.len();
        let max_lag = len / 2;
        let mut best_period = None;
        let mut best_correlation = 0.0;

        for lag in 1..=max_lag {
            let mut correlation = 0.0;
            let mut count = 0;

            for i in 0..(len - lag) {
                correlation += signal[i] * signal[i + lag];
                count += 1;
            }

            correlation /= count as f32;

            if correlation > best_correlation {
                best_correlation = correlation;
                best_period = Some(lag);
            }
        }

        best_period
    }

    /// Cross-validate tempo results from different methods
    fn cross_validate_tempo(&self, comb_bpm: Option<f64>, band_bpms: Vec<f64>) -> f64 {
        let mut candidates = Vec::new();
        
        // Add comb filter result if available
        if let Some(bpm) = comb_bpm {
            candidates.push(bpm);
        }

        // Add band analysis results
        candidates.extend(band_bpms);

        // If we have a previous estimate, include it in validation
        if let Some(prev_bpm) = self.prev_bpm_estimate {
            candidates.push(prev_bpm);
        }

        if candidates.is_empty() {
            return 120.0; // Fallback to default tempo
        }

        // Find the median BPM
        candidates.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median_bpm = candidates[candidates.len() / 2];

        // Calculate weighted average of candidates close to median
        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;

        for &bpm in &candidates {
            let distance = (bpm - median_bpm).abs();
            if distance <= self.config.max_method_deviation {
                let weight = 1.0 / (1.0 + distance);
                weighted_sum += bpm * weight;
                weight_sum += weight;
            }
        }

        if weight_sum > 0.0 {
            weighted_sum / weight_sum
        } else {
            median_bpm
        }
    }

    /// Calculate final confidence based on agreement between methods
    fn calculate_final_confidence(&self, comb_candidates: &[(f64, f64)], final_bpm: f64) -> f64 {
        let mut confidence_factors = Vec::new();

        // Factor 1: Comb filter confidence
        if let Some(&(_, strength)) = comb_candidates.first() {
            confidence_factors.push(strength);
        }

        // Factor 2: Stability of estimate
        if let Some(prev_bpm) = self.prev_bpm_estimate {
            let stability = 1.0 - (final_bpm - prev_bpm).abs() / 10.0;
            confidence_factors.push(stability.max(0.0));
        }

        // Factor 3: Historical confidence
        if !self.confidence_history.is_empty() {
            let hist_confidence = self.confidence_history.iter().sum::<f64>() 
                / self.confidence_history.len() as f64;
            confidence_factors.push(hist_confidence);
        }

        // Calculate weighted average of confidence factors
        let confidence = if confidence_factors.is_empty() {
            0.5 // Default confidence
        } else {
            confidence_factors.iter().sum::<f64>() / confidence_factors.len() as f64
        };

        // Clamp to valid range
        confidence.max(0.0).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn generate_test_signal(bpm: f64, duration_secs: f64, sample_rate: u32) -> Vec<f32> {
        let samples = (duration_secs * sample_rate as f64) as usize;
        let beats_per_second = bpm / 60.0;
        let mut signal = vec![0.0; samples];

        for i in 0..samples {
            let time = i as f64 / sample_rate as f64;
            // Add strong beat at fundamental frequency
            let beat_env = (2.0 * PI * beats_per_second * time).sin().max(0.0);
            signal[i] = beat_env;

            // Add some harmonics
            signal[i] += 0.5 * (4.0 * PI * beats_per_second * time).sin().max(0.0);
            signal[i] += 0.25 * (8.0 * PI * beats_per_second * time).sin().max(0.0);
        }

        // Normalize
        let max_val = signal.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
        signal.iter_mut().for_each(|x| *x /= max_val);

        signal
    }

    #[test]
    fn test_tempo_detection() {
        let sample_rate = 44100;
        let config = MultiMethodTempoConfig::default();
        let mut analyzer = MultiMethodTempoAnalyzer::new(config, sample_rate).unwrap();

        // Test with 120 BPM signal
        let test_bpm = 120.0;
        let signal = generate_test_signal(test_bpm, 5.0, sample_rate);
        let result = analyzer.analyze_tempo(&signal).unwrap();

        // Allow for small margin of error
        assert!((result.bpm - test_bpm).abs() < 1.0);
        assert!(result.confidence > 0.7);
    }

    #[test]
    fn test_tempo_stability() {
        let sample_rate = 44100;
        let config = MultiMethodTempoConfig::default();
        let mut analyzer = MultiMethodTempoAnalyzer::new(config, sample_rate).unwrap();

        // Test with slightly varying BPM
        let base_bpm = 120.0;
        let variations = vec![119.5, 120.2, 120.0, 119.8];

        let mut last_bpm = None;
        for &bpm in &variations {
            let signal = generate_test_signal(bpm, 5.0, sample_rate);
            let result = analyzer.analyze_tempo(&signal).unwrap();

            if let Some(prev_bpm) = last_bpm {
                // Ensure BPM doesn't jump drastically
                assert!((result.bpm - prev_bpm).abs() < 1.0);
            }
            last_bpm = Some(result.bpm);
        }
    }
}
