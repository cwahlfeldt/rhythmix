use crate::common::Result;
use rustfft::{FftPlanner, num_complex::Complex, Fft};
use std::collections::VecDeque;
use std::sync::Arc;

/// Configuration for peak-based tempo analysis
#[derive(Debug, Clone)]
pub struct PeakTempoConfig {
    /// Minimum BPM to detect
    pub min_bpm: f64,
    /// Maximum BPM to detect
    pub max_bpm: f64,
    /// Bands to analyze (low_hz, high_hz)
    pub freq_bands: Vec<(f64, f64)>,
    /// Analysis window size in seconds
    pub window_size: f64,
    /// Minimum peak prominence for detection
    pub peak_threshold: f32,
    /// BPM resolution for search
    pub bpm_resolution: f64,
}

impl PeakTempoConfig {
    pub fn with_range(min_bpm: f64, max_bpm: f64, resolution: f64) -> Self {
        Self {
            min_bpm,
            max_bpm,
            bpm_resolution: resolution,
            freq_bands: vec![
                (20.0, 200.0),    // Bass/kick
                (200.0, 2000.0),  // Mids (snares)
                (2000.0, 8000.0), // High (hats)
            ],
            window_size: 3.0,
            peak_threshold: 0.3,
        }
    }
}

impl Default for PeakTempoConfig {
    fn default() -> Self {
        Self {
            min_bpm: 60.0,
            max_bpm: 200.0,
            freq_bands: vec![
                (20.0, 200.0),    // Bass/kick
                (200.0, 2000.0),  // Mids (snares)
                (2000.0, 8000.0), // High (hats)
            ],
            window_size: 3.0,     // 3 seconds is enough for good resolution
            peak_threshold: 0.3,   // Relative threshold
            bpm_resolution: 0.5,   // Default to 0.5 BPM steps
        }
    }
}

pub struct PeakTempoAnalyzer {
    config: PeakTempoConfig,
    sample_rate: u32,
    fft: Arc<dyn Fft<f32>>,
    band_energies: Vec<VecDeque<f32>>,
    peak_history: VecDeque<(f64, f64)>, // (time, bpm)
}

impl PeakTempoAnalyzer {
    pub fn new(config: PeakTempoConfig, sample_rate: u32) -> Result<Self> {
        let fft_size = 8192; // Good balance of speed and resolution
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);

        Ok(Self {
            config,
            sample_rate,
            fft,
            band_energies: vec![VecDeque::with_capacity(128); 3],
            peak_history: VecDeque::with_capacity(32),
        })
    }

    /// Find peaks in a signal
    fn find_peaks(signal: &[f32], threshold: f32) -> Vec<usize> {
        let mut peaks = Vec::new();
        if signal.len() < 3 {
            return peaks;
        }

        for i in 1..signal.len()-1 {
            if signal[i] > signal[i-1] && signal[i] > signal[i+1] {
                // Check if peak is prominent enough
                let local_min = signal[i-1].min(signal[i+1]);
                let prominence = signal[i] - local_min;
                if prominence > threshold {
                    peaks.push(i);
                }
            }
        }
        peaks
    }

    /// Calculate autocorrelation of a signal
    fn autocorrelate(signal: &[f32], max_lag: usize) -> Vec<f32> {
        let mut correlation = vec![0.0; max_lag];
        let signal_len = signal.len();

        for lag in 0..max_lag {
            let mut sum = 0.0;
            let mut count = 0;
            
            for i in 0..(signal_len - lag) {
                sum += signal[i] * signal[i + lag];
                count += 1;
            }

            if count > 0 {
                correlation[lag] = sum / count as f32;
            }
        }

        correlation
    }

    /// Get tempo candidates from peak intervals
    fn get_tempo_candidates(&self, peaks: &[usize], sample_rate: f32) -> Vec<f64> {
        let mut intervals = Vec::new();
        
        // Calculate intervals between adjacent peaks
        for peaks in peaks.windows(2) {
            let interval = peaks[1] - peaks[0];
            let time = interval as f32 / sample_rate;
            let bpm = 60.0 / time as f64;

            // Consider fundamental and double/half tempo
            for factor in [0.5, 1.0, 2.0] {
                let candidate = bpm * factor;
                if (self.config.min_bpm..=self.config.max_bpm).contains(&candidate) {
                    intervals.push(candidate);
                }
            }
        }

        intervals
    }

    /// Find the most likely tempo from a list of candidates
    fn find_consensus_tempo(&self, candidates: &[f64]) -> Option<(f64, f64)> {
        if candidates.is_empty() {
            return None;
        }

        // Group candidates into clusters
        let mut clusters: Vec<Vec<f64>> = Vec::new();
        let tolerance = 1.0; // BPM tolerance for clustering

        'candidate: for &bpm in candidates {
            for cluster in &mut clusters {
                let mean = cluster.iter().sum::<f64>() / cluster.len() as f64;
                if (bpm - mean).abs() <= tolerance {
                    cluster.push(bpm);
                    continue 'candidate;
                }
            }
            clusters.push(vec![bpm]);
        }

        // Find largest cluster
        if let Some(largest) = clusters.iter().max_by_key(|c| c.len()) {
            let mean = largest.iter().sum::<f64>() / largest.len() as f64;
            let confidence = largest.len() as f64 / candidates.len() as f64;
            return Some((mean, confidence));
        }

        None
    }

    /// Analyze a chunk of audio for tempo
    pub fn analyze_chunk(&mut self, samples: &[f32], time: f64) -> Result<Option<(f64, f64)>> {
        // Convert samples to frequency domain
        let mut fft_buffer: Vec<Complex<f32>> = samples.iter()
            .take(self.fft.len())
            .map(|&x| Complex::new(x, 0.0))
            .collect();
        
        if fft_buffer.len() < self.fft.len() {
            fft_buffer.resize(self.fft.len(), Complex::new(0.0, 0.0));
        }

        self.fft.process(&mut fft_buffer);

        // Calculate energy in each frequency band
        let nyquist = self.sample_rate as f64 / 2.0;
        let freq_per_bin = nyquist / (self.fft.len() / 2) as f64;

        for (band_idx, &(low_hz, high_hz)) in self.config.freq_bands.iter().enumerate() {
            let low_bin = (low_hz / freq_per_bin) as usize;
            let high_bin = ((high_hz / freq_per_bin) as usize).min(self.fft.len() / 2);

            let mut band_energy = 0.0;
            for bin in low_bin..high_bin {
                band_energy += fft_buffer[bin].norm() as f32;
            }

            self.band_energies[band_idx].push_back(band_energy);
            if self.band_energies[band_idx].len() > 128 {
                self.band_energies[band_idx].pop_front();
            }
        }

        // Find peaks in each band and get tempo candidates
        let mut all_candidates = Vec::new();
        
        for energies in &self.band_energies {
            let energy_vec: Vec<f32> = energies.iter().copied().collect();
            let peaks = Self::find_peaks(&energy_vec, self.config.peak_threshold);
            
            // Get tempo candidates from peak intervals
            let candidates = self.get_tempo_candidates(&peaks, 
                self.sample_rate as f32 / (samples.len() as f32 / energies.len() as f32));
            all_candidates.extend(candidates);
        }

        // Find consensus tempo
        if let Some((tempo, confidence)) = self.find_consensus_tempo(&all_candidates) {
            // Store in history
            self.peak_history.push_back((time, tempo));
            if self.peak_history.len() > 32 {
                self.peak_history.pop_front();
            }

            // Stabilize tempo over time
            let stable_tempo = if self.peak_history.len() >= 3 {
                let mut tempos: Vec<_> = self.peak_history.iter().map(|&(_, t)| t).collect();
                tempos.sort_by(|a, b| a.partial_cmp(b).unwrap());
                tempos[tempos.len() / 2] // Use median
            } else {
                tempo
            };

            Ok(Some((stable_tempo, confidence)))
        } else {
            Ok(None)
        }
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
            // Add kick drum (low frequency)
            let kick = (2.0 * PI * 50.0 * time).sin() * 
                      (2.0 * PI * beats_per_second * time).sin().max(0.0).powi(2);
            // Add snare (mid frequency)
            let snare = (2.0 * PI * 200.0 * time).sin() * 
                       (2.0 * PI * beats_per_second * time + PI).sin().max(0.0).powi(2);
            signal[i] = kick * 0.8 + snare * 0.5;
        }

        signal
    }

    #[test]
    fn test_tempo_detection() {
        let sample_rate = 44100;
        let config = PeakTempoConfig::default();
        let mut analyzer = PeakTempoAnalyzer::new(config, sample_rate).unwrap();

        let test_bpms = vec![80.0, 120.0, 140.0, 175.0];
        
        for &test_bpm in &test_bpms {
            let signal = generate_test_signal(test_bpm, 3.0, sample_rate);
            if let Ok(Some((detected_bpm, confidence))) = analyzer.analyze_chunk(&signal, 0.0) {
                assert!((detected_bpm - test_bpm).abs() < 2.0,
                    "Expected BPM {}, got {}", test_bpm, detected_bpm);
                assert!(confidence > 0.5,
                    "Low confidence {} for BPM {}", confidence, test_bpm);
            }
        }
    }
}
