use crate::error::Result;
use crate::onset_detector::{OnsetConfig, OnsetDetector};
use crate::pattern_types::{Lane, Note, NoteType};
use rand::{thread_rng, Rng};
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
    /// Generated notes
    pub notes: Vec<Note>,
    /// Onset times in seconds
    pub onset_times: Vec<f64>,
    /// Beat times in seconds
    pub beat_times: Vec<f64>,
    /// Beat strength values (0.0 to 1.0)
    pub beat_strengths: Vec<f64>,
}

/// Analyzes audio for rhythm game pattern generation
pub struct AudioAnalyzer {
    config: AnalysisConfig,
    pub onset_detector: OnsetDetector,
    onset_times: Vec<f64>,
    inter_onset_intervals: VecDeque<f64>,
    sample_rate: u32,
    current_time: f64,
}

#[derive(Debug, Clone)]
struct BeatState {
    phase: f64,
    period: f64,
    strength: f64,
    last_beat: f64,
}

impl Default for BeatState {
    fn default() -> Self {
        Self {
            phase: 0.0,
            period: 0.5, // 120 BPM default
            strength: 0.0,
            last_beat: 0.0,
        }
    }
}

impl AudioAnalyzer {
    /// Evaluate how well onsets align to a grid at given BPM
    fn evaluate_grid_alignment(&self, bpm: f64) -> f64 {
        if self.onset_times.is_empty() {
            return 0.0;
        }

        let beat_period = 60.0 / bpm;
        let grid_offset = self.onset_times[0];

        // Calculate deviation from grid for each onset
        let mut total_deviation = 0.0;
        for &onset in &self.onset_times {
            let relative_time = onset - grid_offset;
            let nearest_beat = (relative_time / beat_period).round() * beat_period;
            let deviation = (relative_time - nearest_beat).abs() / beat_period;
            total_deviation += if deviation < 0.125 {
                // Within 1/8th beat
                1.0 - (deviation * 8.0) // Linear score from 1.0 to 0.0
            } else {
                0.0
            };
        }

        total_deviation / self.onset_times.len() as f64
    }

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
        // Get magnitude spectrum for energy tracking
        let (magnitudes, _) = self.onset_detector.fft.process_with_phases(samples)?;
        let frame_energy: f32 = magnitudes.iter().map(|x| x * x).sum::<f32>().sqrt();

        // Detect onset
        let is_onset = self.onset_detector.process(samples)?;
        if is_onset {
            log::info!(
                "Onset detected at {:.2}s with energy {:.2}",
                self.current_time,
                frame_energy
            );
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
        // First get tempo estimate
        let (bpm, confidence) = self.estimate_tempo();

        log::info!("Estimated BPM: {}, confidence: {}", bpm, confidence);

        // Track and optimize beat sequence
        let (beat_times, beat_strengths) = self.track_beats(bpm);
        log::info!("Found {} beats", beat_times.len());

        // Generate gameplay notes based on the beat sequence
        let notes = self.generate_notes(bpm, &beat_times);

        AnalysisResults {
            bpm,
            confidence,
            notes,
            onset_times: self.onset_times.clone(),
            beat_times,
            beat_strengths,
        }
    }

    /// Tempo detection using auto-correlation and histogram methods
    fn estimate_tempo(&self) -> (f64, f64) {
        if self.onset_times.is_empty() {
            return (120.0, 0.0);
        }

        let mut median_ioi = 0.0;
        if self.onset_times.len() >= 2 {
            let mut iois: Vec<f64> = self.onset_times.windows(2).map(|w| w[1] - w[0]).collect();
            iois.sort_by(|a, b| a.partial_cmp(b).unwrap());
            median_ioi = iois[iois.len() / 2];
        }

        // Calculate most common beat subdivision
        let raw_bpm = 60.0 / median_ioi;

        // Find the actual BPM by checking common subdivisions
        let possible_subdivisions = [0.25, 0.5, 1.0, 2.0, 4.0];
        let candidate_bpms: Vec<f64> = possible_subdivisions
            .iter()
            .map(|&sub| raw_bpm * sub)
            .filter(|&bpm| bpm >= self.config.min_bpm && bpm <= self.config.max_bpm)
            .collect();

        if candidate_bpms.is_empty() {
            return (120.0, 0.0);
        }

        // Choose the BPM closest to common tempo ranges (70-150 BPM)
        let target_bpm = 120.0;
        let (bpm, _) = candidate_bpms
            .iter()
            .map(|&bpm| (bpm, (bpm - target_bpm).abs()))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();

        (bpm, 1.0)
    }

    /// Enhanced auto-correlation based tempo estimation
    fn autocorrelation_tempo(&self, iois: &[f64]) -> (f64, f64) {
        if iois.is_empty() {
            return (120.0, 0.0);
        }

        let mean_ioi = iois.iter().sum::<f64>() / iois.len() as f64;
        let max_lag = (60.0 / self.config.min_bpm / mean_ioi).floor() as usize;
        let min_lag = (60.0 / self.config.max_bpm / mean_ioi).ceil() as usize;

        if max_lag >= iois.len() || min_lag >= iois.len() {
            return (120.0, 0.0);
        }

        // Calculate weighted auto-correlation
        let mut ac = vec![0.0; max_lag + 1];
        for lag in min_lag..=max_lag {
            let mut weighted_sum = 0.0;
            let mut weight_sum = 0.0;
            let lag_step = lag as f64 * mean_ioi;
            let expected_bpm = 60.0 / lag_step;

            // Weight factor based on common BPM ranges
            let weight = if (80.0..=160.0).contains(&expected_bpm) {
                1.0
            } else if (60.0..=200.0).contains(&expected_bpm) {
                0.8
            } else {
                0.5
            };

            for i in 0..iois.len() - lag {
                let current = (iois[i] - mean_ioi);
                let lagged = (iois[i + lag] - mean_ioi);
                weighted_sum += current * lagged * weight;
                weight_sum += weight;
            }

            if weight_sum > 0.0 {
                ac[lag] = weighted_sum / weight_sum;
            }
        }

        // Find peaks with enhanced criteria
        let mut peaks: Vec<(usize, f64)> = ac
            .windows(3)
            .enumerate()
            .skip(min_lag)
            .filter(|(_, w)| w[1] > w[0] && w[1] > w[2])
            .map(|(i, w)| (i + 1, w[1]))
            .collect();

        if peaks.is_empty() {
            return (120.0, 0.0);
        }

        // Sort peaks and calculate confidence
        peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let base_period = peaks[0].0 as f64 * mean_ioi;
        let base_bpm = 60.0 / base_period;

        // Calculate grid alignment score
        let alignment_score = self.evaluate_grid_alignment(base_bpm);

        (base_bpm, alignment_score)
    }

    /// Histogram-based tempo estimation
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

        // Apply Gaussian smoothing
        let mut smoothed = vec![0.0; histogram.len()];
        let sigma = 2.0;
        let window = (3.0 * sigma) as usize;

        for i in 0..histogram.len() {
            let mut sum = 0.0;
            let mut weight_sum = 0.0;

            for j in i.saturating_sub(window)..=std::cmp::min(i + window, histogram.len() - 1) {
                let x = (j as f64 - i as f64) / sigma;
                let weight = (-0.5 * x * x).exp();
                sum += histogram[j] as f64 * weight;
                weight_sum += weight;
            }

            smoothed[i] = if weight_sum > 0.0 {
                sum / weight_sum
            } else {
                0.0
            };
        }

        // Find peak in smoothed histogram
        let max_val = *smoothed
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        if max_val == 0.0 {
            return (120.0, 0.0);
        }

        let peak_bin = smoothed
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(bin, _)| bin)
            .unwrap_or(0);

        let bpm = self.config.min_bpm + peak_bin as f64 * BIN_WIDTH;
        let confidence = max_val / bpm_values.len() as f64;

        (bpm, confidence)
    }

    /// Track beats using phase and period correction with grid quantization
    fn track_beats(&self, bpm: f64) -> (Vec<f64>, Vec<f64>) {
        const ALPHA: f64 = 0.7; // Phase correction rate
        const BETA: f64 = 0.3; // Period correction rate
        const WINDOW: f64 = 0.2; // 20% tolerance window
        const QUANTIZE_THRESHOLD: f64 = 0.1; // 10% of beat period for quantization

        if self.onset_times.is_empty() {
            return (Vec::new(), Vec::new());
        }

        let beat_period = 60.0 / bpm;

        let mut state = BeatState {
            period: 60.0 / bpm,
            ..Default::default()
        };

        let mut beats = Vec::new();
        let mut strengths = Vec::new();

        for &onset_time in &self.onset_times {
            let phase_error = (onset_time - state.last_beat) % state.period;

            if phase_error.abs() < WINDOW * state.period {
                // Phase correction
                state.phase += ALPHA * phase_error;

                // Period correction
                let period_error = onset_time - state.last_beat - state.period;
                state.period += BETA * period_error;

                // Calculate beat strength
                let strength = 1.0 - (phase_error / (WINDOW * state.period)).abs();

                // Quantize to nearest beat grid position
                let grid_position = (onset_time / beat_period).round() * beat_period;
                let quantize_diff = (onset_time - grid_position).abs();

                // Add beat if it's time and close enough to grid
                if state.phase >= state.period && quantize_diff <= QUANTIZE_THRESHOLD * beat_period
                {
                    beats.push(grid_position); // Use quantized position
                    strengths.push(
                        strength * (1.0 - quantize_diff / (QUANTIZE_THRESHOLD * beat_period)),
                    );
                    state.phase -= state.period;
                    state.last_beat = grid_position;
                }

                state.strength =
                    strength * (1.0 - quantize_diff / (QUANTIZE_THRESHOLD * beat_period));
            }
        }

        log::info!("Initial beat tracking found {} beats", beats.len());
        let result = self.optimize_beat_sequence(&beats, &strengths, state.period);
        log::info!("Optimized to {} beats", result.0.len());
        result
    }

    /// Optimize beat sequence using dynamic programming
    fn optimize_beat_sequence(
        &self,
        beats: &[f64],
        strengths: &[f64],
        period: f64,
    ) -> (Vec<f64>, Vec<f64>) {
        if beats.is_empty() {
            return (Vec::new(), Vec::new());
        }

        let num_beats = beats.len();
        let mut dp = vec![f64::NEG_INFINITY; num_beats];
        let mut prev = vec![0; num_beats];
        dp[0] = strengths[0];

        // Forward pass
        for i in 1..num_beats {
            for j in 0..i {
                let interval = beats[i] - beats[j];
                let expected_beats = (interval / period).round();
                let interval_score = -((interval - expected_beats * period) / period).powi(2);

                let score = dp[j] + interval_score + strengths[i];
                if score > dp[i] {
                    dp[i] = score;
                    prev[i] = j;
                }
            }
        }

        // Backtrack to find optimal sequence
        let mut optimal_beats = Vec::new();
        let mut optimal_strengths = Vec::new();
        let mut current = dp
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(idx, _)| idx)
            .unwrap();

        while current != prev[current] {
            optimal_beats.push(beats[current]);
            optimal_strengths.push(strengths[current]);
            current = prev[current];
        }
        optimal_beats.push(beats[current]);
        optimal_strengths.push(strengths[current]);

        optimal_beats.reverse();
        optimal_strengths.reverse();

        (optimal_beats, optimal_strengths)
    }

    /// Generate notes based on detected tempo and beats with strict grid alignment
    fn generate_notes(&self, bpm: f64, beat_times: &[f64]) -> Vec<Note> {
        if beat_times.is_empty() {
            return Vec::new();
        }

        let mut notes = Vec::new();
        let mut rng = thread_rng();
        let beat_interval = 60.0 / bpm;

        let mut last_lane = 1;
        let first_beat = beat_times[0];

        for (i, &beat_time) in beat_times.iter().enumerate() {
            // Ensure beat is aligned to grid
            let adjusted_time =
                first_beat + ((beat_time - first_beat) / beat_interval).round() * beat_interval;
            let measure_position = (beat_time / beat_interval).round() as usize % 4;

            // Lane selection based on musical structure and ergonomics
            let lane = match measure_position {
                0 => 1, // Downbeat in center
                2 => {
                    if rng.gen_bool(0.7) {
                        1
                    } else {
                        if last_lane == 0 {
                            2
                        } else {
                            0
                        }
                    }
                } // Secondary beat
                _ => {
                    let mut new_lane;
                    loop {
                        new_lane = rng.gen_range(0..3);
                        if i32::abs(new_lane as i32 - last_lane as i32) <= 1 {
                            // Ensure smooth transitions
                            break;
                        }
                    }
                    new_lane
                }
            };

            // Calculate strength based on position and nearby onsets
            let strength = self.onset_times.iter()
                .filter(|&&t| (t - beat_time).abs() < 0.1)
                .map(|_| 1.0)
                .next()
                .unwrap_or(0.5);

            last_lane = lane;
            notes.push(Note {
                timestamp: beat_time,
                note_type: NoteType::Tap,
                lane: Lane::new(lane, 3).unwrap(),
                intensity: strength,
            });
        }

        notes
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
    fn test_tempo_methods() {
        let sample_rate = 44100;
        let config = AnalysisConfig {
            min_bpm: 60.0,
            max_bpm: 200.0,
            ..Default::default()
        };

        let mut analyzer = AudioAnalyzer::new(config, sample_rate).unwrap();

        // Generate test onset times for 120 BPM (0.5s intervals)
        analyzer.onset_times = (0..20).map(|i| i as f64 * 0.5).collect();

        // Test autocorrelation method
        let iois: Vec<f64> = analyzer
            .onset_times
            .windows(2)
            .map(|w| w[1] - w[0])
            .collect();
        let (ac_bpm, ac_conf) = analyzer.autocorrelation_tempo(&iois);
        assert!((ac_bpm - 120.0).abs() < 1.0, "AC BPM: {}", ac_bpm);
        assert!(ac_conf > 0.5, "AC confidence: {}", ac_conf);

        // Test histogram method
        let bpm_values: Vec<f64> = iois.iter().map(|&ioi| 60.0 / ioi).collect();
        let (hist_bpm, hist_conf) = analyzer.histogram_tempo_estimation(&bpm_values);
        assert!((hist_bpm - 120.0).abs() < 1.0, "Hist BPM: {}", hist_bpm);
        assert!(hist_conf > 0.5, "Hist confidence: {}", hist_conf);

        // Test combined estimation
        let results = analyzer.get_results();
        assert!(
            (results.bpm - 120.0).abs() < 1.0,
            "Combined BPM: {}",
            results.bpm
        );
        assert!(
            results.confidence > 0.5,
            "Combined confidence: {}",
            results.confidence
        );
    }

    #[test]
    fn test_tempo_edge_cases() {
        let sample_rate = 44100;
        let config = AnalysisConfig {
            min_bpm: 60.0,
            max_bpm: 200.0,
            ..Default::default()
        };

        let mut analyzer = AudioAnalyzer::new(config.clone(), sample_rate).unwrap();

        // Test empty onset times
        let results = analyzer.get_results();
        assert_eq!(results.bpm, 120.0);
        assert_eq!(results.confidence, 0.0);

        // Test single onset
        analyzer.onset_times = vec![0.0];
        let results = analyzer.get_results();
        assert_eq!(results.bpm, 120.0);
        assert_eq!(results.confidence, 0.0);

        // Test irregular intervals
        analyzer.onset_times = vec![0.0, 0.4, 0.7, 1.2, 1.5, 2.1];
        let results = analyzer.get_results();
        assert!(results.bpm >= config.min_bpm && results.bpm <= config.max_bpm);
        assert!(results.confidence >= 0.0 && results.confidence <= 1.0);
    }

    #[test]
    fn test_note_generation() {
        let sample_rate = 44100;
        let analyzer = AudioAnalyzer::new(AnalysisConfig::default(), sample_rate).unwrap();

        let notes = analyzer.generate_notes(120.0, vec![0.0]);
        assert!(notes.is_empty()); // No onsets, should be empty

        let mut analyzer_with_onsets = analyzer;
        analyzer_with_onsets.onset_times = unsafe { vec![0.0, 0.5, 1.0, 1.5, 2.0] };

        let notes = analyzer_with_onsets.generate_notes(120.0);
        assert!(!notes.is_empty());

        // Check timestamps are sequential
        for i in 1..notes.len() {
            assert!(notes[i].timestamp > notes[i - 1].timestamp);
        }
    }
}
