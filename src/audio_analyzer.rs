use crate::error::Result;
use crate::onset_detector::{OnsetConfig, OnsetDetector};
use crate::types::BeatMarker;
use std::collections::VecDeque;

/// Configuration for audio analysis
#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    /// Configuration for onset detection
    pub onset_config: OnsetConfig,
    /// Minimum BPM to detect
    pub min_bpm: f64,
    /// Maximum BPM to detect
    pub max_bpm: f64,
    /// Window size for tempo estimation (in seconds)
    pub tempo_window: f64,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            onset_config: OnsetConfig::default(),
            min_bpm: 60.0,
            max_bpm: 200.0,
            tempo_window: 5.0,
        }
    }
}

/// Results from audio analysis
#[derive(Debug, Clone)]
pub struct AnalysisResults {
    /// Detected tempo in BPM
    pub bpm: f64,
    /// Confidence in the tempo detection (0.0 to 1.0)
    pub confidence: f64,
    /// Beat markers for visualization
    pub beat_markers: Vec<BeatMarker>,
    /// Onset times in seconds
    pub onset_times: Vec<f64>,
}

/// Analyzes audio for rhythm game pattern generation
pub struct AudioAnalyzer {
    config: AnalysisConfig,
    onset_detector: OnsetDetector,
    onset_times: Vec<f64>,
    inter_onset_intervals: VecDeque<f64>,
    sample_rate: u32,
    current_time: f64,
}

impl AudioAnalyzer {
    /// Creates a new AudioAnalyzer with the specified configuration
    ///
    /// # Arguments
    /// * `config` - Configuration for audio analysis
    /// * `sample_rate` - Sample rate of the audio
    pub fn new(config: AnalysisConfig, sample_rate: u32) -> Result<Self> {
        Ok(Self {
            onset_detector: OnsetDetector::new(config.onset_config.clone(), sample_rate)?,
            config,
            onset_times: Vec::new(),
            inter_onset_intervals: VecDeque::new(),
            sample_rate,
            current_time: 0.0,
        })
    }

    /// Process a chunk of audio samples
    ///
    /// # Arguments
    /// * `samples` - Audio samples to process
    pub fn process_chunk(&mut self, samples: &[f32]) -> Result<()> {
        if self.onset_detector.process(samples)? {
            self.handle_onset();
        }

        self.current_time += samples.len() as f64 / self.sample_rate as f64;
        Ok(())
    }

    /// Handle a detected onset
    fn handle_onset(&mut self) {
        self.onset_times.push(self.current_time);

        // Calculate inter-onset interval if we have at least 2 onsets
        if let Some(&prev_time) = self.onset_times.iter().rev().nth(1) {
            let ioi = self.current_time - prev_time;
            self.inter_onset_intervals.push_back(ioi);

            // Keep a sliding window of intervals
            let window_size = (self.config.tempo_window / ioi).ceil() as usize;
            while self.inter_onset_intervals.len() > window_size {
                self.inter_onset_intervals.pop_front();
            }
        }
    }

    /// Get the analysis results
    pub fn get_results(&self) -> AnalysisResults {
        let (bpm, confidence) = self.estimate_tempo();
        let beat_markers = self.generate_beat_markers(bpm);

        AnalysisResults {
            bpm,
            confidence,
            beat_markers,
            onset_times: self.onset_times.clone(),
        }
    }

    /// Estimate the tempo from inter-onset intervals
    fn estimate_tempo(&self) -> (f64, f64) {
        if self.inter_onset_intervals.is_empty() {
            return (120.0, 0.0); // Default to 120 BPM if no intervals
        }

        // Convert intervals to BPM values
        let bpm_values: Vec<f64> = self
            .inter_onset_intervals
            .iter()
            .map(|&ioi| 60.0 / ioi)
            .filter(|&bpm| bpm >= self.config.min_bpm && bpm <= self.config.max_bpm)
            .collect();

        if bpm_values.is_empty() {
            return (120.0, 0.0);
        }

        // Use histogram method for tempo estimation
        let (bpm, confidence) = self.histogram_tempo_estimation(&bpm_values);
        (bpm, confidence)
    }

    /// Estimate tempo using histogram method
    fn histogram_tempo_estimation(&self, bpm_values: &[f64]) -> (f64, f64) {
        const BIN_WIDTH: f64 = 1.0;
        let num_bins = ((self.config.max_bpm - self.config.min_bpm) / BIN_WIDTH).ceil() as usize;
        let mut histogram = vec![0; num_bins];

        // Fill histogram
        for &bpm in bpm_values {
            let bin = ((bpm - self.config.min_bpm) / BIN_WIDTH).floor() as usize;
            if bin < histogram.len() {
                histogram[bin] += 1;
            }
        }

        // Find peak
        let max_count = *histogram.iter().max().unwrap_or(&0);
        if max_count == 0 {
            return (120.0, 0.0);
        }

        let peak_bin = histogram
            .iter()
            .enumerate()
            .max_by_key(|&(_, &count)| count)
            .map(|(bin, _)| bin)
            .unwrap_or(0);

        let bpm = self.config.min_bpm + peak_bin as f64 * BIN_WIDTH;
        let confidence = max_count as f64 / bpm_values.len() as f64;

        (bpm, confidence)
    }

    /// Generate beat markers based on detected tempo
    fn generate_beat_markers(&self, bpm: f64) -> Vec<BeatMarker> {
        if self.onset_times.is_empty() {
            return Vec::new();
        }

        let beat_interval = 60.0 / bpm;
        let mut markers = Vec::new();
        let mut current_beat = 0;
        let mut current_time = self.onset_times[0];
        let end_time = *self.onset_times.last().unwrap();

        while current_time <= end_time {
            markers.push(BeatMarker {
                timestamp: current_time,
                is_strong_beat: current_beat % 4 == 0,
            });

            current_beat += 1;
            current_time += beat_interval;
        }

        markers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn generate_test_signal(sample_rate: u32, duration: f32, bpm: f64) -> Vec<f32> {
        let num_samples = (sample_rate as f32 * duration) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        let beat_interval = 60.0 / bpm;

        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let beat_phase = (t as f64 / beat_interval) % 1.0;

            // Generate stronger amplitude at beat positions
            let amplitude = if beat_phase < 0.1 { 1.0 } else { 0.5 };
            let sample = amplitude * (2.0 * PI * 440.0 * t).sin();

            samples.push(sample);
        }

        samples
    }

    #[test]
    fn test_tempo_detection() {
        let sample_rate = 44100;
        let config = AnalysisConfig {
            min_bpm: 60.0,
            max_bpm: 200.0,
            ..Default::default()
        };

        let mut analyzer = AudioAnalyzer::new(config, sample_rate).unwrap();
        let test_bpm = 120.0;
        let signal = generate_test_signal(sample_rate, 5.0, test_bpm);

        // Process signal in chunks
        let chunk_size = 1024;
        for chunk in signal.chunks(chunk_size) {
            analyzer.process_chunk(chunk).unwrap();
        }

        let results = analyzer.get_results();

        // Check if detected BPM is within 5% of actual BPM
        assert!((results.bpm - test_bpm).abs() < test_bpm * 0.05);
        assert!(results.confidence > 0.0);
        assert!(!results.beat_markers.is_empty());
    }

    #[test]
    fn test_beat_marker_generation() {
        let sample_rate = 44100;
        let analyzer = AudioAnalyzer::new(AnalysisConfig::default(), sample_rate).unwrap();

        let markers = analyzer.generate_beat_markers(120.0);
        assert!(markers.is_empty()); // No onsets, should be empty

        let mut analyzer_with_onsets = analyzer;
        analyzer_with_onsets.onset_times = unsafe { vec![0.0, 0.5, 1.0, 1.5, 2.0] };

        let markers = analyzer_with_onsets.generate_beat_markers(120.0);
        assert!(!markers.is_empty());

        // Check if strong beats are correctly marked
        let strong_beats: Vec<_> = markers.iter().filter(|m| m.is_strong_beat).collect();
        assert!(!strong_beats.is_empty());
    }
}
