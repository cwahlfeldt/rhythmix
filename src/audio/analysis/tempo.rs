use crate::common::Result;
use std::collections::VecDeque;

/// Configuration for tempo detection
#[derive(Debug, Clone)]
pub struct TempoConfig {
    /// Minimum BPM to detect
    pub min_bpm: f64,
    /// Maximum BPM to detect
    pub max_bpm: f64,
    /// Window size for tempo estimation (in seconds)
    pub tempo_window: f64,
    /// Minimum confidence level for tempo detection
    pub confidence_threshold: f64,
}

impl Default for TempoConfig {
    fn default() -> Self {
        Self {
            min_bpm: 60.0,
            max_bpm: 200.0,
            tempo_window: 5.0,
            confidence_threshold: 0.5,
        }
    }
}

/// Results from tempo analysis
#[derive(Debug, Clone)]
pub struct TempoResults {
    /// Detected tempo in BPM
    pub bpm: f64,
    /// Confidence level of the detection (0.0 - 1.0)
    pub confidence: f64,
    /// Phase offset in seconds
    pub phase: f64,
}

/// Analyzes tempo and beat information in audio
pub struct TempoAnalyzer {
    config: TempoConfig,
    inter_onset_intervals: VecDeque<f64>,
    current_time: f64,
    sample_rate: u32,
}

impl TempoAnalyzer {
    /// Creates a new TempoAnalyzer with the specified configuration
    pub fn new(config: TempoConfig, sample_rate: u32) -> Self {
        Self {
            config,
            inter_onset_intervals: VecDeque::new(),
            current_time: 0.0,
            sample_rate,
        }
    }

    /// Process an onset event and update tempo estimation
    pub fn process_onset(&mut self, onset_time: f64) -> Result<Option<TempoResults>> {
        // Calculate inter-onset interval if we have previous onsets
        if !self.inter_onset_intervals.is_empty() {
            let ioi = onset_time - self.current_time;
            
            // Only add if it's within a reasonable range
            let min_ioi = 60.0 / self.config.max_bpm;
            let max_ioi = 60.0 / self.config.min_bpm;
            
            if ioi >= min_ioi && ioi <= max_ioi {
                self.inter_onset_intervals.push_back(ioi);

                // Keep a sliding window of intervals
                let window_size = (self.config.tempo_window / ioi).ceil() as usize;
                while self.inter_onset_intervals.len() > window_size {
                    self.inter_onset_intervals.pop_front();
                }

                // Only estimate tempo if we have enough intervals
                if self.inter_onset_intervals.len() >= 4 {
                    return Ok(Some(self.estimate_tempo()));
                }
            }
        }

        self.current_time = onset_time;
        Ok(None)
    }

    /// Estimate tempo from collected inter-onset intervals
    fn estimate_tempo(&self) -> TempoResults {
        // Convert intervals to BPM values
        let mut bpms: Vec<f64> = self.inter_onset_intervals
            .iter()
            .map(|&ioi| 60.0 / ioi)
            .collect();

        bpms.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Use median as primary tempo estimate
        let median_bpm = bpms[bpms.len() / 2];

        // Calculate confidence based on consistency of intervals
        let mut confidence = 0.0;
        let tolerance = 2.0; // BPM tolerance for considering intervals consistent

        for bpm in &bpms {
            if (bpm - median_bpm).abs() <= tolerance {
                confidence += 1.0;
            }
        }
        confidence /= bpms.len() as f64;

        // Calculate phase based on most recent onset
        let phase = self.current_time % (60.0 / median_bpm);

        TempoResults {
            bpm: median_bpm,
            confidence,
            phase,
        }
    }

    /// Update the current time based on processed samples
    pub fn advance_time(&mut self, num_samples: usize) {
        self.current_time += num_samples as f64 / self.sample_rate as f64;
    }

    /// Get the last detected tempo and confidence
    pub fn get_last_tempo(&self) -> Option<(f64, f64)> {
        if !self.inter_onset_intervals.is_empty() {
            let tempo = self.estimate_tempo();
            Some((tempo.bpm, tempo.confidence))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_tempo_estimation() {
        let sample_rate = 44100;
        let config = TempoConfig {
            min_bpm: 60.0,
            max_bpm: 200.0,
            tempo_window: 5.0,
            confidence_threshold: 0.5,
        };

        let mut analyzer = TempoAnalyzer::new(config, sample_rate);

        // Simulate regular onsets at 120 BPM
        let interval = 60.0 / 120.0; // 0.5 seconds between beats
        let mut time = 0.0;

        for _ in 0..8 {
            // Add some random jitter to make it more realistic
            let jitter = (rand::random::<f64>() - 0.5) * 0.01;
            time += interval + jitter;

            if let Ok(Some(results)) = analyzer.process_onset(time) {
                // Allow for some variance due to jitter
                assert!((results.bpm - 120.0).abs() < 5.0);
                assert!(results.confidence > 0.5);
                assert!(results.phase >= 0.0 && results.phase < interval);
            }
        }
    }

    #[test]
    fn test_tempo_bounds() {
        let sample_rate = 44100;
        let config = TempoConfig {
            min_bpm: 80.0,
            max_bpm: 160.0,
            tempo_window: 5.0,
            confidence_threshold: 0.5,
        };

        let mut analyzer = TempoAnalyzer::new(config, sample_rate);

        // Test intervals that should be ignored (too fast/slow)
        let too_fast = 60.0 / 200.0; // 200 BPM
        let too_slow = 60.0 / 40.0;  // 40 BPM

        assert!(analyzer.process_onset(0.0).unwrap().is_none());
        assert!(analyzer.process_onset(too_fast).unwrap().is_none());
        assert!(analyzer.process_onset(too_slow).unwrap().is_none());
    }
}
