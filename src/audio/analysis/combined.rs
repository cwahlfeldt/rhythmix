use super::{
    comb_filter::{CombFilterAnalyzer, CombFilterConfig},
    features::{AudioFeatures, FeatureConfig, FeatureExtractor},
    onset::{OnsetConfig, OnsetDetector, OnsetResult},
    tempo::{TempoAnalyzer, TempoConfig, TempoResults},
};
use crate::common::Result;

/// Configuration for combined analysis
#[derive(Debug, Clone)]
pub struct CombinedConfig {
    pub onset: OnsetConfig,
    pub tempo: TempoConfig,
    pub features: FeatureConfig,
    pub comb: CombFilterConfig,
    pub onset_weight: f64,
    pub tempo_weight: f64,
    pub comb_weight: f64,
}

impl Default for CombinedConfig {
    fn default() -> Self {
        Self {
            onset: OnsetConfig::default(),
            tempo: TempoConfig::default(),
            features: FeatureConfig::default(),
            comb: CombFilterConfig::default(),
            onset_weight: 0.4,
            tempo_weight: 0.3,
            comb_weight: 0.3,
        }
    }
}

/// Combined analysis results
#[derive(Debug, Clone)]
pub struct CombinedResults {
    pub onset: Option<OnsetResult>,
    pub tempo: Option<TempoResults>,
    pub features: Option<AudioFeatures>,
    pub confidence: f64,
}

pub struct CombinedAnalyzer {
    config: CombinedConfig,
    onset_detector: OnsetDetector,
    tempo_analyzer: TempoAnalyzer,
    feature_extractor: FeatureExtractor,
    comb_analyzer: CombFilterAnalyzer,
    sample_rate: u32,
    beat_history: Vec<f64>,
}

impl CombinedAnalyzer {
    pub fn new(config: CombinedConfig, sample_rate: u32) -> Result<Self> {
        // Ensure all configs use the same window size
        let window_size = config.onset.window_size;
        let mut features_config = config.features.clone();
        features_config.window_size = window_size;

        Ok(Self {
            onset_detector: OnsetDetector::new(config.onset.clone())?,
            tempo_analyzer: TempoAnalyzer::new(config.tempo.clone(), sample_rate),
            feature_extractor: FeatureExtractor::new(features_config, sample_rate)?,
            comb_analyzer: CombFilterAnalyzer::new(config.comb.clone(), sample_rate)?,
            config,
            sample_rate,
            beat_history: Vec::new(),
        })
    }

    /// Get the window size used by this analyzer
    pub fn window_size(&self) -> usize {
        self.config.onset.window_size
    }

    pub fn process(&mut self, samples: &[f32], time: f64) -> Result<CombinedResults> {
        // Get results from each analyzer
        let onset_result = self.onset_detector.process(samples)?;
        let mut tempo_update = None;
        let features = self.feature_extractor.process(samples)?;

        // Only run tempo analysis if we detect an onset
        if onset_result.is_onset {
            tempo_update = self.tempo_analyzer.process_onset(time, onset_result.strength as f64)?;
            self.beat_history.push(time);
            
            // Keep history manageable
            while !self.beat_history.is_empty() && time - self.beat_history[0] > 5.0 {
                self.beat_history.remove(0);
            }
        }

        // Run comb filter analysis periodically
        let mut comb_confidence = 0.0;
        if !self.beat_history.is_empty() {
            let comb_results = self.comb_analyzer.analyze(samples)?;
            if let Some((_, correlation)) = comb_results
                .iter()
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            {
                comb_confidence = self.comb_analyzer.correlation_to_confidence(*correlation);
            }
        }

        // Combine confidences
        let onset_conf = if onset_result.is_onset {
            onset_result.strength as f64
        } else {
            0.0
        };

        let tempo_conf = tempo_update
            .as_ref()
            .map(|t| t.confidence)
            .unwrap_or(0.0);

        // Weight and combine confidences with adaptive weighting
        self.adapt_weights(&features);
        
        let combined_confidence = 
            (onset_conf * self.config.onset_weight +
            tempo_conf * self.config.tempo_weight +
            comb_confidence * self.config.comb_weight) /
            (self.config.onset_weight + self.config.tempo_weight + self.config.comb_weight);

        // Calculate additional features for beat classification
        let is_bass_heavy = features.bass_energy > 0.6;
        let is_transient_heavy = features.zero_crossing_rate > 0.3;

        // Apply feature-based confidence adjustments
        let mut feature_adjusted_confidence = match (is_bass_heavy, is_transient_heavy) {
            (true, _) => combined_confidence * 1.2,  // Boost confidence for bass-heavy beats
            (false, true) => combined_confidence * 1.1,  // Slight boost for transient-heavy beats
            _ => combined_confidence,
        };

        // Apply dynamic confidence threshold based on recent history
        if let Some(predicted) = self.predict_next_beat() {
            let timing_error = (time - predicted).abs();
            if timing_error < 0.05 {  // Within 50ms of predicted beat
                feature_adjusted_confidence *= 1.2;
            } else if timing_error > 0.1 {  // More than 100ms off predicted beat
                feature_adjusted_confidence *= 0.8;
            }
        }

        // Ensure confidence stays in valid range
        feature_adjusted_confidence = feature_adjusted_confidence.min(1.0).max(0.0);

        Ok(CombinedResults {
            onset: Some(onset_result),
            tempo: tempo_update,
            features: Some(features),
            confidence: feature_adjusted_confidence,
        })
    }

    /// Get predicted next beat time based on current analysis
    pub fn predict_next_beat(&self) -> Option<f64> {
        if let Some(last_beat) = self.beat_history.last() {
            if let Some((bpm, _)) = self.tempo_analyzer.get_last_tempo() {
                let beat_interval = 60.0 / bpm;
                return Some(last_beat + beat_interval);
            }
        }
        None
    }

    /// Check if a given time falls within a beat window
    pub fn is_within_beat_window(&self, time: f64, window_size: f64) -> bool {
        if let Some(predicted) = self.predict_next_beat() {
            (time - predicted).abs() <= window_size
        } else {
            false
        }
    }

    /// Adjust analysis weights based on audio characteristics
    fn adapt_weights(&mut self, features: &AudioFeatures) {
        // Increase onset weight for transient-heavy content
        if features.zero_crossing_rate > 0.4 {
            self.config.onset_weight = 0.5;
            self.config.tempo_weight = 0.3;
            self.config.comb_weight = 0.2;
        }
        // Increase tempo weight for consistent rhythmic content
        else if features.bass_energy > 0.5 {
            self.config.onset_weight = 0.3;
            self.config.tempo_weight = 0.4;
            self.config.comb_weight = 0.3;
        }
        // Balance weights for other content
        else {
            self.config.onset_weight = 0.4;
            self.config.tempo_weight = 0.3;
            self.config.comb_weight = 0.3;
        }
    }

    /// Analyze beat consistency over recent history
    fn get_beat_consistency(&self) -> f64 {
        if self.beat_history.len() < 2 {
            return 0.0;
        }

        let intervals: Vec<f64> = self.beat_history.windows(2)
            .map(|w| w[1] - w[0])
            .collect();

        let mean_interval = intervals.iter().sum::<f64>() / intervals.len() as f64;
        let variance = intervals.iter()
            .map(|&i| (i - mean_interval).powi(2))
            .sum::<f64>() / intervals.len() as f64;

        (-variance * 10.0).exp() // Convert variance to consistency score (0-1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn generate_test_signal(duration_secs: f64, sample_rate: u32, bpm: f64) -> Vec<f32> {
        let samples = (duration_secs * sample_rate as f64) as usize;
        let period_samples = (60.0 * sample_rate as f64 / bpm) as usize;
        let mut signal = vec![0.0; samples];

        for i in 0..samples {
            // Add periodic beats with realistic envelope
            if i % period_samples < (sample_rate as usize / 50) {
                let envelope = 0.5 * (1.0 - (2.0 * PI * i as f32 / (sample_rate as f32 / 50.0)).cos());
                let bass = (2.0 * PI * 60.0 * i as f32 / sample_rate as f32).sin();
                let mid = 0.5 * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin();
                signal[i] = envelope * (bass + mid);
            }
        }

        signal
    }

    #[test]
    fn test_combined_analysis() {
        let sample_rate = 44100;
        let config = CombinedConfig::default();
        let mut analyzer = CombinedAnalyzer::new(config, sample_rate).unwrap();

        let test_bpm = 120.0;
        let signal = generate_test_signal(5.0, sample_rate, test_bpm);
        let mut detected_beats = 0;
        let mut last_beat_time = 0.0;

        // Process signal in windows
        let window_size = analyzer.window_size();
        let hop_size = window_size / 2;  // 50% overlap
        let mut pos = 0;

        while pos + window_size <= signal.len() {
            let time = pos as f64 / sample_rate as f64;
            let window = &signal[pos..pos + window_size];
            let result = analyzer.process(window, time).unwrap();

            if result.confidence > 0.6 {
                if last_beat_time == 0.0 || time - last_beat_time >= 0.25 {
                    detected_beats += 1;
                    last_beat_time = time;
                }
            }

            pos += hop_size;
        }

        // Verify we detected a reasonable number of beats
        let expected_beats = (5.0 * test_bpm / 60.0) as i32;
        let margin = 2;
        assert!(
            (detected_beats - expected_beats).abs() <= margin,
            "Expected {} beats (±{}), got {}",
            expected_beats,
            margin,
            detected_beats
        );
    }

    #[test]
    fn test_beat_consistency() {
        let sample_rate = 44100;
        let config = CombinedConfig::default();
        let mut analyzer = CombinedAnalyzer::new(config, sample_rate).unwrap();

        // Add some regular beats
        for i in 0..5 {
            analyzer.beat_history.push(i as f64 * 0.5); // Regular 120 BPM beats
        }

        let consistency = analyzer.get_beat_consistency();
        assert!(consistency > 0.9, "Beat consistency should be high for regular beats");
    }

    #[test]
    fn test_feature_based_adaptation() {
        let sample_rate = 44100;
        let config = CombinedConfig::default();
        let mut analyzer = CombinedAnalyzer::new(config, sample_rate).unwrap();

        let mut features = AudioFeatures {
            rms: 0.5,
            centroid: 1000.0,
            spread: 500.0,
            bass_energy: 0.8,  // High bass energy
            mid_energy: 0.3,
            high_energy: 0.1,
            rolloff: 2000.0,
            zero_crossing_rate: 0.2,
        };

        // Test adaptation to bass-heavy content
        analyzer.adapt_weights(&features);
        assert!(analyzer.config.tempo_weight > analyzer.config.onset_weight, 
            "Tempo weight should be higher for bass-heavy content");

        // Test adaptation to transient-heavy content
        features.bass_energy = 0.3;
        features.zero_crossing_rate = 0.5;
        analyzer.adapt_weights(&features);
        assert!(analyzer.config.onset_weight > analyzer.config.tempo_weight,
            "Onset weight should be higher for transient-heavy content");
    }
}
