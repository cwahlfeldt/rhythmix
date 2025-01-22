use crate::audio_analyzer::AnalysisResults;
use crate::difficulty::{DifficultyConfig, DifficultyManager};
use crate::error::Result;
use crate::pattern_types::{Lane, Note, NoteType, PatternSection};
use crate::types::{PatternData, PatternMetadata};
use rand::{thread_rng, Rng};
use std::time::Duration;

/// Configuration for pattern generation
#[derive(Debug, Clone)]
pub struct GeneratorConfig {
    /// Number of lanes in the game
    pub lane_count: u8,
    /// Difficulty configuration
    pub difficulty: DifficultyConfig,
    /// Minimum time between pattern repetitions
    pub min_pattern_repeat_time: Duration,
    /// Maximum consecutive notes in same lane
    pub max_same_lane_repeat: u8,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            lane_count: 3,
            difficulty: DifficultyConfig::default(),
            min_pattern_repeat_time: Duration::from_secs(4),
            max_same_lane_repeat: 2,
        }
    }
}

/// Generates rhythm game patterns from audio analysis
pub struct PatternGenerator {
    config: GeneratorConfig,
    difficulty: DifficultyManager,
}

impl PatternGenerator {
    /// Creates a new PatternGenerator
    ///
    /// # Arguments
    /// * `config` - Generator configuration
    /// * `bpm` - Detected BPM of the audio
    pub fn new(config: GeneratorConfig, bpm: f64) -> Result<Self> {
        let difficulty = DifficultyManager::new(config.difficulty.clone(), bpm)?;
        Ok(Self { config, difficulty })
    }

    /// Generates a pattern from audio analysis results
    ///
    /// # Arguments
    /// * `analysis` - Results from audio analysis
    pub fn generate_pattern(&mut self, analysis: &AnalysisResults) -> Result<PatternData> {
        let mut notes = Vec::new();
        let mut rng = thread_rng();

        // Create a note for each beat marker
        for beat_marker in &analysis.beat_markers {
            // Generate a random lane for the note
            let lane_num = rng.gen_range(0..self.config.lane_count);
            let lane = Lane::new(lane_num, self.config.lane_count)?;

            // Create a note at the beat marker's timestamp
            let note = Note {
                timestamp: beat_marker.timestamp,
                note_type: NoteType::Tap,
                lane,
            };

            notes.push(note);
        }

        // Create a simple section for the whole song
        let sections = vec![PatternSection {
            start_time: 0.0,
            end_time: analysis
                .beat_markers
                .last()
                .map(|m| m.timestamp)
                .unwrap_or(0.0),
            section_type: "main".to_string(),
            intensity: 0.8,
        }];

        Ok(PatternData {
            metadata: PatternMetadata {
                bpm: analysis.bpm,
                duration: *analysis.onset_times.last().unwrap_or(&0.0),
                difficulty: self.difficulty.calculate_difficulty(),
                recommended_scroll_speed: self.difficulty.get_recommended_scroll_speed(),
            },
            notes,
            sections,
            beat_markers: analysis.beat_markers.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_analysis() -> AnalysisResults {
        AnalysisResults {
            bpm: 120.0,
            confidence: 0.9,
            beat_markers: vec![],
            onset_times: unsafe { vec![0.0, 0.5, 1.0, 1.5, 2.0] },
        }
    }

    #[test]
    fn test_pattern_generation() {
        let config = GeneratorConfig::default();
        let mut generator = PatternGenerator::new(config, 120.0).unwrap();
        let analysis = create_test_analysis();

        let pattern = generator.generate_pattern(&analysis).unwrap();

        assert!(!pattern.notes.is_empty());
        assert!(!pattern.sections.is_empty());
        assert_eq!(pattern.metadata.bpm, 120.0);
    }

    #[test]
    fn test_note_spacing() {
        let config = GeneratorConfig::default();
        let mut generator = PatternGenerator::new(config, 120.0).unwrap();
        let analysis = create_test_analysis();

        let pattern = generator.generate_pattern(&analysis).unwrap();

        // Check minimum spacing between notes
        for i in 1..pattern.notes.len() {
            let time_diff = pattern.notes[i].timestamp - pattern.notes[i - 1].timestamp;
            assert!(time_diff >= 0.0);
        }
    }

    #[test]
    fn test_lane_distribution() {
        let config = GeneratorConfig::default();
        let mut generator = PatternGenerator::new(config, 120.0).unwrap();
        let analysis = create_test_analysis();

        let pattern = generator.generate_pattern(&analysis).unwrap();

        // Check that notes use different lanes
        let mut used_lanes = Vec::new();
        for note in &pattern.notes {
            used_lanes.push(note.lane.value());
        }
        used_lanes.sort_unstable();
        used_lanes.dedup();
        assert!(used_lanes.len() > 1); // Should use more than one lane
    }
}
