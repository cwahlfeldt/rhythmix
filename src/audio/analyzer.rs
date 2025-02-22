use crate::common::Result;
use crate::audio::analysis::{
    OnsetDetector, OnsetConfig,
    TempoAnalyzer, TempoConfig,
    FeatureExtractor, FeatureConfig,
    AudioFeatures,
};
use crate::audio::analysis::tempo::TimeSignature;

/// Configuration for complete audio analysis
#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    /// Configuration for onset detection
    pub onset_config: OnsetConfig,
    /// Configuration for tempo analysis
    pub tempo_config: TempoConfig,
    /// Configuration for feature extraction
    pub feature_config: FeatureConfig,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            onset_config: OnsetConfig::default(),
            // Use DnB-specific tempo config
            tempo_config: TempoConfig::for_genre("dnb"),
            feature_config: FeatureConfig::default(),
        }
    }
}

/// Results from complete audio analysis
#[derive(Debug, Clone)]
pub struct AnalysisResults {
    /// Detected tempo in BPM
    pub bpm: f64,
    /// Confidence in tempo detection (0.0 - 1.0)
    pub tempo_confidence: f64,
    /// Detected time signature
    pub time_signature: TimeSignature,
    /// Onset times in seconds
    pub onset_times: Vec<f64>,
    /// Onset strengths (0.0 - 1.0)
    pub onset_strengths: Vec<f32>,
    /// Audio features at onset points
    pub onset_features: Vec<AudioFeatures>,
    /// Average features across the whole file
    pub avg_features: Option<AudioFeatures>,
}

/// Comprehensive audio analyzer that combines onset detection, tempo analysis, and feature extraction
pub struct AudioAnalyzer {
    #[allow(dead_code)]
    config: AnalysisConfig,  // Stored for future dynamic reconfiguration
    onset_detector: OnsetDetector,
    tempo_analyzer: TempoAnalyzer,
    feature_extractor: FeatureExtractor,
    onset_times: Vec<f64>,
    onset_strengths: Vec<f32>,
    onset_features: Vec<AudioFeatures>,
    feature_sum: AudioFeatureAccumulator,
    frame_count: usize,
    current_time: f64,
    sample_rate: u32,
}

/// Helper struct for accumulating audio feature averages
#[derive(Debug, Default)]
struct AudioFeatureAccumulator {
    rms: f32,
    centroid: f32,
    spread: f32,
    bass_energy: f32,
    mid_energy: f32,
    high_energy: f32,
    rolloff: f32,
    zero_crossing_rate: f32,
}

impl AudioFeatureAccumulator {
    fn add(&mut self, features: &AudioFeatures) {
        self.rms += features.rms;
        self.centroid += features.centroid;
        self.spread += features.spread;
        self.bass_energy += features.bass_energy;
        self.mid_energy += features.mid_energy;
        self.high_energy += features.high_energy;
        self.rolloff += features.rolloff;
        self.zero_crossing_rate += features.zero_crossing_rate;
    }

    fn compute_average(&self, count: usize) -> Option<AudioFeatures> {
        if count == 0 {
            None
        } else {
            let factor = 1.0 / count as f32;
            Some(AudioFeatures {
                rms: self.rms * factor,
                centroid: self.centroid * factor,
                spread: self.spread * factor,
                bass_energy: self.bass_energy * factor,
                mid_energy: self.mid_energy * factor,
                high_energy: self.high_energy * factor,
                rolloff: self.rolloff * factor,
                zero_crossing_rate: self.zero_crossing_rate * factor,
            })
        }
    }
}

impl AudioAnalyzer {
    /// Creates a new AudioAnalyzer with the specified configuration
    pub fn new(config: AnalysisConfig, sample_rate: u32) -> Result<Self> {
        Ok(Self {
            onset_detector: OnsetDetector::new(config.onset_config.clone())?,
            tempo_analyzer: TempoAnalyzer::new(config.tempo_config.clone(), sample_rate),
            feature_extractor: FeatureExtractor::new(config.feature_config.clone(), sample_rate)?,
            config,
            onset_times: Vec::new(),
            onset_strengths: Vec::new(),
            onset_features: Vec::new(),
            feature_sum: AudioFeatureAccumulator::default(),
            frame_count: 0,
            current_time: 0.0,
            sample_rate,
        })
    }

    /// Process a chunk of audio samples
    pub fn process_chunk(&mut self, samples: &[f32]) -> Result<()> {
        // Detect onsets
        let onset_result = self.onset_detector.process(samples)?;
        
        // Extract audio features
        let features = self.feature_extractor.process(samples)?;
        
        // Accumulate features for averaging
        self.feature_sum.add(&features);
        self.frame_count += 1;

        // If we detected an onset, update tempo and store features
        if onset_result.is_onset {
            // Update tempo analysis
            if let Ok(Some(tempo_result)) = self.tempo_analyzer.process_onset(self.current_time, onset_result.strength as f64) {
                log::info!(
                    "Tempo update at {:.2}s - BPM: {:.1}, Confidence: {:.2}",
                    self.current_time,
                    tempo_result.bpm,
                    tempo_result.confidence
                );
                
                // Check current tempo state
                if let Some((current_bpm, current_conf)) = self.tempo_analyzer.get_last_tempo() {
                    log::info!(
                        "Current tempo state - BPM: {:.1}, Confidence: {:.2}",
                        current_bpm,
                        current_conf
                    );
                }
            }

            // Store onset information
            self.onset_times.push(self.current_time);
            self.onset_strengths.push(onset_result.strength);
            self.onset_features.push(features);

            log::info!(
                "Onset detected at {:.2}s with strength {:.2}",
                self.current_time,
                onset_result.strength
            );
        }

        // Update current time
        self.current_time += samples.len() as f64 / self.sample_rate as f64;
        
        Ok(())
    }

    /// Get the current analysis state
    pub fn get_current_state(&self) -> AnalysisResults {
        // Get the latest tempo analysis
        let (bpm, confidence) = if let Some((detected_bpm, detected_conf)) = self.tempo_analyzer.get_last_tempo() {
            // If we have a detected tempo and it's reasonable, use it
            if (165.0..=185.0).contains(&detected_bpm) || detected_conf > 0.6 {
                (detected_bpm, detected_conf)
            } else {
                // Outside DnB range, check if it's a multiple
                let halved = detected_bpm / 2.0;
                let doubled = detected_bpm * 2.0;
                
                if (165.0..=185.0).contains(&halved) {
                    (halved, detected_conf * 0.9)
                } else if (165.0..=185.0).contains(&doubled) {
                    (doubled, detected_conf * 0.9)
                } else {
                    // If nothing reasonable found, default to DnB tempo
                    (174.0, 0.3)
                }
            }
        } else {
            (174.0, 0.3)  // Default only if no tempo detected
        };

        AnalysisResults {
            bpm,
            tempo_confidence: confidence,
            time_signature: TimeSignature::default(),
            onset_times: self.onset_times.clone(),
            onset_strengths: self.onset_strengths.clone(),
            onset_features: self.onset_features.clone(),
            avg_features: self.feature_sum.compute_average(self.frame_count),
        }
    }

    /// Get the final analysis results
    pub fn get_results(&mut self) -> AnalysisResults {
        self.get_current_state()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn generate_test_signal(sample_rate: u32, duration: f32) -> Vec<f32> {
        let num_samples = (sample_rate as f32 * duration) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        // Generate a signal with clear onsets and frequency changes
        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let base_freq = if (t * 4.0).floor() % 2.0 == 0.0 {
                440.0 // A4
            } else {
                880.0 // A5
            };

            // Add some harmonics
            let sample = 0.5 * (2.0 * PI * base_freq * t).sin() +
                        0.3 * (2.0 * PI * base_freq * 2.0 * t).sin() +
                        0.2 * (2.0 * PI * base_freq * 3.0 * t).sin();
            
            samples.push(sample);
        }

        samples
    }

    #[test]
    fn test_audio_analysis() {
        let sample_rate = 44100;
        let config = AnalysisConfig::default();
        let mut analyzer = AudioAnalyzer::new(config, sample_rate).unwrap();

        // Generate and analyze 1 second of audio
        let signal = generate_test_signal(sample_rate, 1.0);
        
        // Process in chunks
        let chunk_size = 1024;
        for chunk in signal.chunks(chunk_size) {
            if chunk.len() == chunk_size {
                analyzer.process_chunk(chunk).unwrap();
            }
        }

        let results = analyzer.get_results();

        // Verify results
        assert!(!results.onset_times.is_empty(), "No onsets detected");
        assert_eq!(results.onset_times.len(), results.onset_strengths.len());
        assert_eq!(results.onset_times.len(), results.onset_features.len());
        assert!(results.avg_features.is_some());
        
        // Verify tempo is reasonable
        assert!(results.bpm >= 60.0 && results.bpm <= 200.0);

        // Verify onset timings
        if results.onset_times.len() >= 2 {
            for window in results.onset_times.windows(2) {
                let spacing = window[1] - window[0];
                assert!(spacing >= 0.1); // At least 100ms between onsets
            }
        }

        // Verify feature extraction
        if let Some(avg_features) = results.avg_features {
            assert!(avg_features.rms > 0.0);
            assert!(avg_features.centroid > 0.0);
            assert!(avg_features.bass_energy + avg_features.mid_energy + avg_features.high_energy > 0.0);
        }
    }
}
