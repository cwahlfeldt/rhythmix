use crate::common::Result;
use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::Arc;

/// Configuration for comb filter analysis
#[derive(Debug, Clone)]
pub struct CombFilterConfig {
    /// Minimum BPM to analyze
    pub min_bpm: f64,
    /// Maximum BPM to analyze
    pub max_bpm: f64,
    /// BPM resolution (steps between BPM values)
    pub bpm_resolution: f64,
    /// Analysis window size in seconds
    pub window_size: f64,
}

impl Default for CombFilterConfig {
    fn default() -> Self {
        Self {
            min_bpm: 50.0,
            max_bpm: 220.0,
            bpm_resolution: 0.5,
            window_size: 1.0,  // Changed to 1 second to better match other analyzers
        }
    }
}

pub struct CombFilterAnalyzer {
    config: CombFilterConfig,
    sample_rate: u32,
    window_samples: usize,
    fft: Arc<dyn rustfft::Fft<f32>>,
}

impl CombFilterAnalyzer {
    pub fn new(config: CombFilterConfig, sample_rate: u32) -> Result<Self> {
        let window_samples = (config.window_size * sample_rate as f64) as usize;
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(window_samples);

        Ok(Self {
            config,
            sample_rate,
            window_samples,
            fft,
        })
    }

    /// Analyze a section of audio using comb filter method
    pub fn analyze(&self, samples: &[f32]) -> Result<Vec<(f64, f64)>> {
        let mut bpm_strengths = Vec::new();
        
        // For each BPM value
        let mut bpm = self.config.min_bpm;
        while bpm <= self.config.max_bpm {
            let period_samples = (60.0 * self.sample_rate as f64 / bpm) as usize;
            
            // Create comb filter signal
            let mut comb = vec![0.0f32; self.window_samples];
            let mut pos = 0;
            while pos < self.window_samples {
                comb[pos] = 1.0;
                pos += period_samples;
            }

            // Apply FFT to both signals
            let signal_fft = self.compute_fft(samples);
            let comb_fft = self.compute_fft(&comb);

            // Compute correlation
            let correlation = self.compute_correlation(&signal_fft, &comb_fft);
            
            bpm_strengths.push((bpm, correlation));
            bpm += self.config.bpm_resolution;
        }

        Ok(bpm_strengths)
    }

    /// Compute FFT of a signal
    fn compute_fft(&self, signal: &[f32]) -> Vec<Complex<f32>> {
        let mut buffer: Vec<Complex<f32>> = signal
            .iter()
            .map(|&x| Complex::new(x, 0.0))
            .collect();

        self.fft.process(&mut buffer);
        buffer
    }

    /// Compute correlation between two FFT results
    fn compute_correlation(&self, signal_fft: &[Complex<f32>], comb_fft: &[Complex<f32>]) -> f64 {
        let mut correlation = 0.0;
        
        for (s, c) in signal_fft.iter().zip(comb_fft.iter()) {
            correlation += (s * c.conj()).norm() as f64;
        }

        correlation / self.window_samples as f64
    }

    /// Convert correlation value to confidence score
    pub fn correlation_to_confidence(&self, correlation: f64) -> f64 {
        // Normalize and scale correlation to confidence value
        let min_correlation = 0.1;
        let max_correlation = 2.0;
        
        ((correlation - min_correlation) / (max_correlation - min_correlation))
            .max(0.0)
            .min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn generate_test_signal(bpm: f64, duration_secs: f64, sample_rate: u32) -> Vec<f32> {
        let samples = (duration_secs * sample_rate as f64) as usize;
        let period_samples = (60.0 * sample_rate as f64 / bpm) as usize;
        
        let mut signal = vec![0.0; samples];
        let mut pos = 0;
        
        while pos < samples {
            // Add beat
            let beat_width = sample_rate as usize / 50; // 20ms beat
            for i in 0..beat_width.min(samples - pos) {
                let env = 0.5 * (1.0 - (2.0 * PI * i as f32 / beat_width as f32).cos());
                signal[pos + i] = env;
            }
            pos += period_samples;
        }
        
        signal
    }

    #[test]
    fn test_comb_filter_analysis() {
        let sample_rate = 44100;
        let config = CombFilterConfig::default();
        let analyzer = CombFilterAnalyzer::new(config.clone(), sample_rate).unwrap();

        // Test with 120 BPM signal
        let test_bpm = 120.0;
        let signal = generate_test_signal(test_bpm, config.window_size, sample_rate);
        let results = analyzer.analyze(&signal).unwrap();

        // Find strongest BPM
        let (detected_bpm, _) = results
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();

        // Allow for small margin of error
        assert!((detected_bpm - test_bpm).abs() < 2.0);
    }
}
