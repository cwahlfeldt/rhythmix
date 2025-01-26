use std::collections::VecDeque;
use crate::audio::analysis::AudioFeatures;
use crate::common::Result;
use crate::pattern::types::SectionType;

/// Configuration for audio-based section detection
#[derive(Debug, Clone)]
pub struct SectionDetectionConfig {
    /// Window size for energy history
    pub history_size: usize,
    /// Minimum duration between sections (seconds)
    pub min_section_duration: f64,
    /// Novelty score threshold for boundary detection
    pub novelty_threshold: f64,
    /// Moving average window size for thresholding
    pub ma_window_size: usize,
}

impl Default for SectionDetectionConfig {
    fn default() -> Self {
        Self {
            history_size: 32,
            min_section_duration: 8.0,
            novelty_threshold: 0.3,
            ma_window_size: 8,
        }
    }
}

/// Represents a section boundary in the audio
#[derive(Debug, Clone)]
pub struct SectionBoundary {
    /// Time of the boundary in seconds
    pub time: f64,
    /// Novelty score that triggered the boundary
    pub novelty_score: f64,
    /// Detected section type
    pub boundary_type: SectionType,
    /// Confidence in the boundary detection (0.0 - 1.0)
    pub confidence: f64,
}

/// Helper struct for calculating moving averages
#[derive(Debug)]
struct MovingAverage {
    values: VecDeque<f32>,
    sum: f32,
    window_size: usize,
}

impl MovingAverage {
    fn new(window_size: usize) -> Self {
        Self {
            values: VecDeque::with_capacity(window_size),
            sum: 0.0,
            window_size,
        }
    }

    fn add(&mut self, value: f32) {
        self.sum += value;
        self.values.push_back(value);

        if self.values.len() > self.window_size {
            if let Some(old_value) = self.values.pop_front() {
                self.sum -= old_value;
            }
        }
    }

    fn mean(&self) -> f32 {
        if self.values.is_empty() {
            0.0
        } else {
            self.sum / self.values.len() as f32
        }
    }
}

/// Detects section boundaries based on audio features
#[derive(Debug)]
pub struct AudioSectionDetector {
    config: SectionDetectionConfig,
    energy_history: VecDeque<AudioFeatures>,
    bass_ma: MovingAverage,
    mid_ma: MovingAverage,
    high_ma: MovingAverage,
    novelty_scores: VecDeque<f64>,
    section_boundaries: Vec<SectionBoundary>,
    current_time: f64,
    last_section_time: f64,
}

impl AudioSectionDetector {
    /// Creates a new AudioSectionDetector with the specified configuration
    pub fn new(config: SectionDetectionConfig) -> Self {
        Self {
            energy_history: VecDeque::with_capacity(config.history_size),
            bass_ma: MovingAverage::new(config.ma_window_size),
            mid_ma: MovingAverage::new(config.ma_window_size),
            high_ma: MovingAverage::new(config.ma_window_size),
            novelty_scores: VecDeque::with_capacity(config.ma_window_size),
            section_boundaries: Vec::new(),
            current_time: 0.0,
            last_section_time: 0.0,
            config,
        }
    }

    /// Process new audio features and detect section boundaries
    pub fn process_features(&mut self, features: &AudioFeatures, time: f64) -> Result<Option<SectionBoundary>> {
        // Update state
        self.current_time = time;
        self.energy_history.push_back(features.clone());
        if self.energy_history.len() > self.config.history_size {
            self.energy_history.pop_front();
        }

        // Update moving averages
        self.bass_ma.add(features.bass_energy);
        self.mid_ma.add(features.mid_energy);
        self.high_ma.add(features.high_energy);

        // Calculate novelty score
        let novelty = self.calculate_novelty_score(features);
        self.novelty_scores.push_back(novelty);
        if self.novelty_scores.len() > self.config.ma_window_size {
            self.novelty_scores.pop_front();
        }

        // Check for section boundary
        if self.is_section_boundary(novelty) {
            let boundary_type = self.classify_section_boundary(features);
            let confidence = self.calculate_boundary_confidence(novelty, features);
            
            let boundary = SectionBoundary {
                time,
                novelty_score: novelty,
                boundary_type,
                confidence,
            };

            self.section_boundaries.push(boundary.clone());
            self.last_section_time = time;

            Ok(Some(boundary))
        } else {
            Ok(None)
        }
    }

    /// Calculate novelty score based on multiple factors
    fn calculate_novelty_score(&self, features: &AudioFeatures) -> f64 {
        // Energy changes across frequency bands
        let energy_change = self.calculate_energy_change(features);
        
        // Spectral contrast
        let spectral_contrast = self.calculate_spectral_contrast(features);
        
        // Combined score with weightings
        0.6 * energy_change + 0.4 * spectral_contrast
    }

    /// Calculate energy change across frequency bands
    fn calculate_energy_change(&self, features: &AudioFeatures) -> f64 {
        let bass_diff = (features.bass_energy - self.bass_ma.mean()).abs() as f64;
        let mid_diff = (features.mid_energy - self.mid_ma.mean()).abs() as f64;
        let high_diff = (features.high_energy - self.high_ma.mean()).abs() as f64;

        // Weight lower frequencies more heavily as they often indicate structural changes
        (bass_diff * 0.5 + mid_diff * 0.3 + high_diff * 0.2)
            .clamp(0.0, 1.0)
    }

    /// Calculate spectral contrast between bands
    fn calculate_spectral_contrast(&self, features: &AudioFeatures) -> f64 {
        let bass_mid_contrast = (features.bass_energy - features.mid_energy).abs() as f64;
        let mid_high_contrast = (features.mid_energy - features.high_energy).abs() as f64;
        
        ((bass_mid_contrast + mid_high_contrast) / 2.0)
            .clamp(0.0, 1.0)
    }

    /// Determine if a novelty score indicates a section boundary
    fn is_section_boundary(&self, novelty: f64) -> bool {
        // Check minimum time between sections
        if self.current_time - self.last_section_time < self.config.min_section_duration {
            return false;
        }

        // Calculate adaptive threshold
        let threshold = if self.novelty_scores.is_empty() {
            self.config.novelty_threshold
        } else {
            let mean: f64 = self.novelty_scores.iter().sum::<f64>() / self.novelty_scores.len() as f64;
            mean * (1.0 + self.config.novelty_threshold)
        };

        // Check if novelty exceeds threshold
        novelty > threshold
    }

    /// Classify the type of section based on audio features
    fn classify_section_boundary(&self, features: &AudioFeatures) -> SectionType {
        let bass_intensity = features.bass_energy;
        let mid_intensity = features.mid_energy;
        let high_intensity = features.high_energy;
        let total_energy = bass_intensity + mid_intensity + high_intensity;

        // High energy across spectrum often indicates chorus
        if total_energy > 0.8 && bass_intensity > 0.3 && mid_intensity > 0.3 {
            SectionType::Chorus
        }
        // Strong bass and mids, less highs might be verse
        else if bass_intensity > 0.3 && mid_intensity > 0.3 && high_intensity < 0.2 {
            SectionType::Verse
        }
        // High contrast between bands might indicate bridge
        else if (bass_intensity - high_intensity).abs() > 0.5 {
            SectionType::Bridge
        }
        // Low energy across spectrum could be break or intro
        else if total_energy < 0.3 {
            if self.section_boundaries.is_empty() {
                SectionType::Intro
            } else {
                SectionType::Bridge
            }
        }
        // Default to verse if no clear characteristics
        else {
            SectionType::Verse
        }
    }

    /// Calculate confidence in boundary detection
    fn calculate_boundary_confidence(&self, novelty: f64, features: &AudioFeatures) -> f64 {
        // Combine multiple confidence factors:
        
        // 1. Novelty score relative to threshold
        let novelty_confidence = {
            let mean_novelty = self.novelty_scores.iter().sum::<f64>() / self.novelty_scores.len().max(1) as f64;
            ((novelty / mean_novelty) - 1.0).clamp(0.0, 1.0)
        };

        // 2. Feature consistency in recent history
        let feature_confidence = if self.energy_history.len() >= 2 {
            let prev_features = &self.energy_history[self.energy_history.len() - 2];
            let energy_diff = (features.rms - prev_features.rms).abs();
            (1.0 - energy_diff as f64).clamp(0.0, 1.0)
        } else {
            0.5 // Default if not enough history
        };

        // Combine confidence scores with weights
        (novelty_confidence * 0.7 + feature_confidence * 0.3)
            .clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_features(bass: f32, mid: f32, high: f32, rms: f32) -> AudioFeatures {
        AudioFeatures {
            rms,
            centroid: 0.0,
            spread: 0.0,
            bass_energy: bass,
            mid_energy: mid,
            high_energy: high,
            rolloff: 0.0,
            zero_crossing_rate: 0.0,
        }
    }

    #[test]
    fn test_section_detection() {
        let config = SectionDetectionConfig {
            history_size: 8,
            min_section_duration: 2.0,
            novelty_threshold: 0.3,
            ma_window_size: 4,
        };

        let mut detector = AudioSectionDetector::new(config);

        // Test verse features
        let verse_features = create_test_features(0.4, 0.4, 0.2, 0.5);
        let result = detector.process_features(&verse_features, 0.0).unwrap();
        assert!(result.is_none()); // First feature shouldn't create boundary

        // Test transition to chorus
        let chorus_features = create_test_features(0.8, 0.8, 0.7, 0.9);
        let result = detector.process_features(&chorus_features, 3.0).unwrap();
        assert!(result.is_some());
        if let Some(boundary) = result {
            assert_eq!(boundary.boundary_type, SectionType::Chorus);
            assert!(boundary.confidence > 0.5);
        }

        // Test bridge features
        let bridge_features = create_test_features(0.3, 0.3, 0.8, 0.6);
        let result = detector.process_features(&bridge_features, 6.0).unwrap();
        assert!(result.is_some());
        if let Some(boundary) = result {
            assert_eq!(boundary.boundary_type, SectionType::Bridge);
        }
    }

    #[test]
    fn test_minimum_section_duration() {
        let config = SectionDetectionConfig {
            min_section_duration: 4.0,
            ..Default::default()
        };

        let mut detector = AudioSectionDetector::new(config);

        // Process initial features
        let features1 = create_test_features(0.4, 0.4, 0.2, 0.5);
        let _ = detector.process_features(&features1, 0.0).unwrap();

        // Try to create boundary too soon
        let features2 = create_test_features(0.8, 0.8, 0.7, 0.9);
        let result = detector.process_features(&features2, 2.0).unwrap();
        assert!(result.is_none()); // Should not create boundary before min duration

        // Try after minimum duration
        let result = detector.process_features(&features2, 5.0).unwrap();
        assert!(result.is_some()); // Should create boundary after min duration
    }
}
