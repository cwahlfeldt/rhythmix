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
pub struct TimeSignature {
    /// Number of beats per measure
    pub beats_per_measure: u8,
    /// Note value that represents one beat (4 = quarter, 8 = eighth, etc.)
    pub beat_unit: u8,
    /// Confidence in the time signature detection (0.0 - 1.0)
    pub confidence: f64,
}

impl Default for TimeSignature {
    fn default() -> Self {
        Self {
            beats_per_measure: 4, // Default to 4/4
            beat_unit: 4,
            confidence: 0.0,
        }
    }
}

#[derive(Clone)]
pub struct TempoResults {
    /// Detected tempo in BPM
    pub bpm: f64,
    /// Confidence level of the detection (0.0 - 1.0)
    pub confidence: f64,
    /// Phase offset in seconds
    /// Phase offset - used for future beat alignment
    pub phase: f64,
    /// Detected time signature
    pub time_signature: TimeSignature,
}

/// Helper struct for BPM clustering
#[derive(Debug)]
struct BpmCluster {
    bpm: f64,
    values: Vec<f64>,
    confidence: f64,
}

impl BpmCluster {
    fn new(initial_bpm: f64) -> Self {
        Self {
            bpm: initial_bpm,
            values: Vec::new(),
            confidence: 0.0,
        }
    }

    fn add_value(&mut self, value: f64) {
        self.values.push(value);
        self.recalculate_stats();
    }

    fn merge(&mut self, other: &BpmCluster) {
        self.values.extend(&other.values);
        self.recalculate_stats();
    }

    fn recalculate_stats(&mut self) {
        if !self.values.is_empty() {
            // Use median for BPM
            let mut sorted_values = self.values.clone();
            sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
            self.bpm = sorted_values[sorted_values.len() / 2];

            // Calculate confidence based on consistency
            let mut variance_sum = 0.0;
            for &value in &self.values {
                variance_sum += (value - self.bpm).powi(2);
            }
            let variance = variance_sum / self.values.len() as f64;

            // Convert variance to confidence (0.0 - 1.0)
            self.confidence = (-variance / 4.0).exp(); // exponential falloff
        }
    }
}

/// Analyzes tempo and beat information in audio
pub struct TempoAnalyzer {
    config: TempoConfig,
    inter_onset_intervals: VecDeque<f64>,
    current_time: f64,
    beat_strengths: VecDeque<f64>, // Store beat strengths for time signature detection
    measure_positions: VecDeque<usize>, // Store beat positions within measures
    #[allow(dead_code)]
    sample_rate: u32,
    last_tempo: Option<TempoResults>,
}

impl TempoAnalyzer {
    /// Creates a new TempoAnalyzer with the specified configuration
    pub fn new(config: TempoConfig, sample_rate: u32) -> Self {
        Self {
            config,
            inter_onset_intervals: VecDeque::new(),
            current_time: 0.0,
            beat_strengths: VecDeque::with_capacity(32),
            measure_positions: VecDeque::with_capacity(32),
            sample_rate,
            last_tempo: None,
        }
    }

    /// Detect time signature based on beat strengths and positions
    fn detect_time_signature(&self) -> TimeSignature {
        if self.beat_strengths.len() < 8 {
            return TimeSignature::default();
        }

        // Calculate average strengths for each beat position
        let mut position_strengths = vec![0.0; 12]; // Support up to 12/X time signatures
        let mut position_counts = vec![0; 12];

        for (pos, &strength) in self
            .measure_positions
            .iter()
            .zip(self.beat_strengths.iter())
        {
            position_strengths[*pos] += strength;
            position_counts[*pos] += 1;
        }

        // Normalize strengths
        for i in 0..12 {
            if position_counts[i] > 0 {
                position_strengths[i] /= position_counts[i] as f64;
            }
        }

        // Find peaks in position strengths
        let peaks: Vec<_> = position_strengths
            .iter()
            .enumerate()
            .filter(|&(i, &strength)| {
                let prev = if i == 0 {
                    0.0
                } else {
                    position_strengths[i - 1]
                };
                let next = if i == 11 {
                    0.0
                } else {
                    position_strengths[i + 1]
                };
                strength > 0.0 && strength >= prev && strength >= next
            })
            .collect();

        // Determine beats per measure based on peak spacing
        let (beats_per_measure, confidence) = if peaks.len() >= 2 {
            let spacings: Vec<_> = peaks.windows(2).map(|w| w[1].0 - w[0].0).collect();

            // Most common spacing indicates the measure length
            let mut spacing_counts = std::collections::HashMap::new();
            for &spacing in &spacings {
                *spacing_counts.entry(spacing).or_insert(0) += 1;
            }

            let most_common = spacing_counts
                .into_iter()
                .max_by_key(|&(_, count)| count)
                .unwrap_or((4, 0));

            let confidence = if spacings.is_empty() {
                0.0
            } else {
                most_common.1 as f64 / spacings.len() as f64
            };

            (most_common.0 as u8, confidence)
        } else {
            (4, 0.0) // Default to 4/4 with low confidence
        };

        // Look for triplet feel
        let triplet_ratio = self
            .inter_onset_intervals
            .iter()
            .filter(|&&ioi| {
                let quarter = 60.0 / self.last_tempo.as_ref().map(|t| t.bpm).unwrap_or(120.0);
                (ioi - quarter / 3.0).abs() < quarter / 12.0
            })
            .count() as f64
            / self.inter_onset_intervals.len() as f64;

        let beat_unit = if triplet_ratio > 0.3 { 8 } else { 4 };

        TimeSignature {
            beats_per_measure,
            beat_unit,
            confidence,
        }
    }

    /// Process an onset event and update tempo estimation
    pub fn process_onset(
        &mut self,
        onset_time: f64,
        strength: f64,
    ) -> Result<Option<TempoResults>> {
        let mut tempo_update = None;

        // Only process if onset time is after current time
        if onset_time >= self.current_time {
            // Calculate inter-onset interval
            if self.current_time > 0.0 {
                let ioi = onset_time - self.current_time;

                // Only add if it's within a reasonable range
                let min_ioi = 60.0 / self.config.max_bpm;
                let max_ioi = 60.0 / self.config.min_bpm;

                if ioi >= min_ioi && ioi <= max_ioi {
                    self.inter_onset_intervals.push_back(ioi);

                    // Keep a sliding window of intervals
                    let window_size = (self.config.tempo_window / min_ioi).ceil() as usize;
                    while self.inter_onset_intervals.len() > window_size {
                        self.inter_onset_intervals.pop_front();
                    }

                    // Only estimate tempo if we have enough intervals
                    if self.inter_onset_intervals.len() >= 4 {
                        // Store beat strength and position
                        if let Some(last_tempo) = &self.last_tempo {
                            let beat_position = ((onset_time * last_tempo.bpm / 60.0)
                                % last_tempo.time_signature.beats_per_measure as f64)
                                .round() as usize;
                            self.beat_strengths.push_back(strength);
                            self.measure_positions.push_back(beat_position);

                            // Keep history manageable
                            if self.beat_strengths.len() > 32 {
                                self.beat_strengths.pop_front();
                                self.measure_positions.pop_front();
                            }
                        }

                        let mut new_tempo = self.estimate_tempo();
                        new_tempo.time_signature = self.detect_time_signature();

                        // Update if confidence is good enough or we don't have a previous estimate
                        if new_tempo.confidence >= self.config.confidence_threshold
                            || self.last_tempo.is_none()
                        {
                            self.last_tempo = Some(new_tempo.clone());
                            tempo_update = Some(new_tempo);
                        }
                    }
                }
            }

            self.current_time = onset_time;
        }

        Ok(tempo_update)
    }

    /// Get the last detected tempo and confidence
    pub fn get_last_tempo(&self) -> Option<(f64, f64)> {
        self.last_tempo.as_ref().map(|t| (t.bpm, t.confidence))
    }

    /// Update the current time based on processed samples
    #[allow(dead_code)]
    pub fn advance_time(&mut self, num_samples: usize) {
        // Used for future real-time processing
        self.current_time += num_samples as f64 / self.sample_rate as f64;
    }

    /// Estimate tempo from collected inter-onset intervals
    fn estimate_tempo(&self) -> TempoResults {
        // First get all possible BPM values
        let mut bpms: Vec<f64> = self
            .inter_onset_intervals
            .iter()
            .map(|&ioi| 60.0 / ioi)
            .collect();

        // Sort BPMs for clustering analysis
        bpms.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Find clusters of similar BPM values
        let mut clusters = self.find_bpm_clusters(&bpms);

        // Use the largest, most consistent cluster
        clusters.sort_by(|a, b| {
            let a_score = a.values.len() as f64 * a.confidence;
            let b_score = b.values.len() as f64 * b.confidence;
            b_score.partial_cmp(&a_score).unwrap()
        });

        let best_cluster = &clusters[0];
        let phase = self.current_time % (60.0 / best_cluster.bpm);

        TempoResults {
            bpm: best_cluster.bpm,
            confidence: best_cluster.confidence,
            phase,
            time_signature: self.detect_time_signature(),
        }
    }

    /// Find clusters of similar BPM values
    fn find_bpm_clusters(&self, bpms: &[f64]) -> Vec<BpmCluster> {
        let mut clusters: Vec<BpmCluster> = Vec::new();
        let tolerance = 2.0; // BPM tolerance for clustering

        for &bpm in bpms {
            // Try to add to existing cluster
            let mut added = false;
            for cluster in &mut clusters {
                if (cluster.bpm - bpm).abs() <= tolerance {
                    cluster.add_value(bpm);
                    added = true;
                    break;
                }
            }

            // Create new cluster if needed
            if !added {
                let mut cluster = BpmCluster::new(bpm);
                cluster.add_value(bpm);
                clusters.push(cluster);
            }
        }

        // Merge overlapping clusters
        let mut i = 0;
        while i < clusters.len() {
            let mut j = i + 1;
            while j < clusters.len() {
                if (clusters[i].bpm - clusters[j].bpm).abs() <= tolerance * 1.5 {
                    let removed = clusters.remove(j);
                    clusters[i].merge(&removed);
                } else {
                    j += 1;
                }
            }
            i += 1;
        }

        // Ensure at least one cluster
        if clusters.is_empty() {
            let mut default_cluster = BpmCluster::new(120.0);
            default_cluster.confidence = 0.1;
            clusters.push(default_cluster);
        }

        clusters
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

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
        let mut rng = rand::thread_rng();

        // Simulate regular onsets at 120 BPM with some jitter
        let interval = 60.0 / 120.0; // 0.5 seconds between beats
        let mut time = 0.0;

        for _ in 0..12 {
            let jitter = rng.gen_range(-0.01..0.01); // ±10ms jitter
            time += interval + jitter;

            if let Ok(Some(results)) = analyzer.process_onset(time) {
                // Allow for some variance due to jitter
                assert!(
                    (results.bpm - 120.0).abs() < 5.0,
                    "BPM estimate {}",
                    results.bpm
                );
                assert!(
                    results.confidence > 0.5,
                    "Low confidence: {}",
                    results.confidence
                );
                assert!(results.phase >= 0.0 && results.phase < interval);
            }
        }

        // Verify final tempo through get_last_tempo
        if let Some((bpm, confidence)) = analyzer.get_last_tempo() {
            assert!((bpm - 120.0).abs() < 5.0);
            assert!(confidence > 0.5);
        } else {
            panic!("No tempo detected");
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
        let too_slow = 60.0 / 40.0; // 40 BPM

        assert!(analyzer.process_onset(0.0).unwrap().is_none());
        assert!(analyzer.process_onset(too_fast).unwrap().is_none());
        assert!(analyzer.process_onset(too_slow).unwrap().is_none());
    }

    #[test]
    fn test_bpm_clustering() {
        let sample_rate = 44100;
        let config = TempoConfig::default();
        let mut analyzer = TempoAnalyzer::new(config, sample_rate);

        // Simulate two close but distinct tempos
        let bpm1 = 120.0;
        let bpm2 = 123.0; // Close but different

        let mut time = 0.0;
        let mut rng = rand::thread_rng();

        // Add alternating tempos
        for i in 0..20 {
            let base_bpm = if i % 2 == 0 { bpm1 } else { bpm2 };
            let jitter = rng.gen_range(-0.01..0.01);
            time += 60.0 / base_bpm + jitter;

            let _ = analyzer.process_onset(time);
        }

        // Check that clustering found a reasonable middle ground
        if let Some((bpm, confidence)) = analyzer.get_last_tempo() {
            assert!(bpm > bpm1.min(bpm2) && bpm < bpm1.max(bpm2));
            assert!(confidence > 0.5);
        } else {
            panic!("No tempo detected");
        }
    }
}
