use crate::audio::analysis::{
    tempo::TimeSignature, AudioFeatures, FeatureConfig, FeatureExtractor, OnsetConfig,
    OnsetDetector, PeakTempoAnalyzer, PeakTempoConfig,
};
use crate::common::Result;

/// Configuration for complete audio analysis
#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    /// Configuration for onset detection
    pub onset_config: OnsetConfig,
    /// Configuration for tempo analysis
    pub tempo_config: PeakTempoConfig,
    /// Configuration for feature extraction
    pub feature_config: FeatureConfig,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            onset_config: OnsetConfig::default(),
            tempo_config: PeakTempoConfig::default(),
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

pub struct AudioAnalyzer {
    #[allow(dead_code)]
    config: AnalysisConfig,
    onset_detector: OnsetDetector,
    tempo_analyzer: PeakTempoAnalyzer,
    feature_extractor: FeatureExtractor,
    onset_times: Vec<f64>,
    onset_strengths: Vec<f32>,
    onset_features: Vec<AudioFeatures>,
    feature_sum: AudioFeatureAccumulator,
    frame_count: usize,
    current_time: f64,
    sample_rate: u32,
    current_window: Vec<f32>,
    window_size: usize,
    last_tempo: Option<(f64, f64)>, // (bpm, confidence),
    tempo_history: Vec<(f64, f64, f64)>,
}

impl AudioAnalyzer {
    /// Creates a new AudioAnalyzer with the specified configuration
    pub fn new(config: AnalysisConfig, sample_rate: u32) -> Result<Self> {
        let window_size = (config.tempo_config.window_size * sample_rate as f64) as usize;
        Ok(Self {
            onset_detector: OnsetDetector::new(config.onset_config.clone())?,
            tempo_analyzer: PeakTempoAnalyzer::new(config.tempo_config.clone(), sample_rate)?,
            feature_extractor: FeatureExtractor::new(config.feature_config.clone(), sample_rate)?,
            config,
            onset_times: Vec::new(),
            onset_strengths: Vec::new(),
            onset_features: Vec::new(),
            feature_sum: AudioFeatureAccumulator::default(),
            frame_count: 0,
            current_time: 0.0,
            sample_rate,
            current_window: Vec::with_capacity(window_size),
            window_size,
            last_tempo: None,
            tempo_history: Vec::new(),
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

        // Add samples to tempo analysis window
        self.current_window.extend_from_slice(samples);

        // Process complete windows with overlap
        if self.current_window.len() >= self.window_size {
            if let Ok(Some((bpm, confidence))) = self
                .tempo_analyzer
                .analyze_chunk(&self.current_window, self.current_time)
            {
                self.last_tempo = Some((bpm, confidence));
                self.tempo_history
                    .push((self.current_time, bpm, confidence));
                log::info!(
                    "First pass tempo analysis at {:.2}s - BPM: {:.1}, Confidence: {:.2}",
                    self.current_time,
                    bpm,
                    confidence
                );
            }

            // Keep 1/4 window overlap
            let retain = self.window_size / 4;
            self.current_window
                .drain(0..self.current_window.len() - retain);
        }

        // Store onset information
        if onset_result.is_onset {
            self.onset_times.push(self.current_time);
            self.onset_strengths.push(onset_result.strength);
            self.onset_features.push(features);
        }

        // Update current time
        self.current_time += samples.len() as f64 / self.sample_rate as f64;

        Ok(())
    }

    /// Get the final analysis results with two-pass BPM detection
    pub fn get_results(&mut self) -> Result<AnalysisResults> {
        // First pass: collect all our data points
        let mut tempo_points = Vec::new();

        // Include our running history
        if let Some((bpm, conf)) = self.last_tempo {
            tempo_points.push((bpm, conf));
        }

        // Process final window
        if !self.current_window.is_empty() {
            if let Ok(Some((bpm, conf))) = self
                .tempo_analyzer
                .analyze_chunk(&self.current_window, self.current_time)
            {
                tempo_points.push((bpm, conf));
            }
        }

        // Get initial estimate from first pass
        let initial_estimate = if !tempo_points.is_empty() {
            // Weight by confidence and take average
            let sum_weights: f64 = tempo_points.iter().map(|(_, conf)| conf).sum();
            let weighted_bpm: f64 = tempo_points
                .iter()
                .map(|(bpm, conf)| bpm * conf)
                .sum::<f64>()
                / sum_weights;
            weighted_bpm
        } else {
            120.0 // fallback
        };

        log::info!("Starting second pass analysis...");
        log::info!("First pass BPM estimate: {:.2}", initial_estimate);

        // Store all track data for analysis
        let total_duration = self.current_time;
        let full_data = self.current_window.clone();
        log::info!(
            "Starting full track analysis: {:.2} seconds of audio",
            total_duration
        );

        let refined_config = PeakTempoConfig::with_range(
            (initial_estimate - 5.0).max(60.0), // ±5 BPM range
            (initial_estimate + 5.0).min(200.0),
            0.1, // 1/10th BPM resolution
        );

        log::info!(
            "Second pass config: BPM range {:.1}-{:.1} with {:.3} resolution",
            refined_config.min_bpm,
            refined_config.max_bpm,
            refined_config.bpm_resolution
        );

        // Create temporary analyzer for second pass
        let mut refined_analyzer = PeakTempoAnalyzer::new(refined_config, self.sample_rate)
            .map_err(|e| {
                log::error!("Failed to create refined analyzer: {}", e);
                e
            })?;

        // Use multiple windows for analysis
        let mut refined_estimates = Vec::new();
        let window_size = self.window_size;
        let hop_size = window_size / 4; // 75% overlap for more detail
        let num_windows = (full_data.len() - window_size) / hop_size + 1;

        log::info!(
            "Analyzing {} windows with {} samples each...",
            num_windows,
            window_size
        );

        let mut start = 0;
        while start + window_size <= full_data.len() {
            let window = &full_data[start..start + window_size];
            if let Ok(Some((bpm, conf))) =
                refined_analyzer.analyze_chunk(window, start as f64 / self.sample_rate as f64)
            {
                log::debug!(
                    "Window at {:.2}s: BPM = {:.2}, confidence = {:.2}",
                    start as f64 / self.sample_rate as f64,
                    bpm,
                    conf
                );
                refined_estimates.push((bpm, conf));
            }
            start += hop_size;
        }

        log::info!(
            "Second pass found {} valid BPM estimates",
            refined_estimates.len()
        );

        // Calculate final BPM from refined estimates
        let (final_bpm, final_confidence) = if !refined_estimates.is_empty() {
            // Sort by confidence first
            refined_estimates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            // Take top 25% most confident estimates
            let top_n = (refined_estimates.len() / 4).max(1);
            let top_estimates: Vec<_> = refined_estimates.iter().take(top_n).collect();

            // Calculate weighted average of top estimates
            let total_confidence: f64 = top_estimates.iter().map(|(_, c)| c).sum();
            let weighted_bpm = top_estimates
                .iter()
                .map(|(bpm, conf)| bpm * conf)
                .sum::<f64>()
                / total_confidence;

            (weighted_bpm, refined_estimates[0].1) // Use highest confidence
        } else {
            log::warn!("No refined estimates found, using initial estimate");
            (initial_estimate, 0.3)
        };

        if !refined_estimates.is_empty() {
            log::info!("Second pass analysis summary:");
            log::info!("  - Windows analyzed: {}", refined_estimates.len());
            log::info!(
                "  - BPM range found: {:.2} - {:.2}",
                refined_estimates
                    .iter()
                    .map(|(bpm, _)| bpm)
                    .fold(f64::INFINITY, |a, b| a.min(*b)),
                refined_estimates
                    .iter()
                    .map(|(bpm, _)| bpm)
                    .fold(f64::NEG_INFINITY, |a, b| a.max(*b))
            );
        }
        log::info!(
            "Final refined BPM: {:.2} (confidence: {:.2})",
            final_bpm,
            final_confidence
        );

        Ok(AnalysisResults {
            bpm: final_bpm,
            tempo_confidence: final_confidence,
            time_signature: TimeSignature::default(),
            onset_times: self.onset_times.clone(),
            onset_strengths: self.onset_strengths.clone(),
            onset_features: self.onset_features.clone(),
            avg_features: self.feature_sum.compute_average(self.frame_count),
        })
    }
}
