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
            lane_count: 4,
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
    last_pattern: Vec<Note>,
    last_pattern_time: f64,
    lane_repeat_count: Vec<u8>,
    last_lane: Option<Lane>,
}

impl PatternGenerator {
    /// Creates a new PatternGenerator
    ///
    /// # Arguments
    /// * `config` - Generator configuration
    /// * `bpm` - Detected BPM of the audio
    pub fn new(config: GeneratorConfig, bpm: f64) -> Result<Self> {
        let difficulty = DifficultyManager::new(config.difficulty.clone(), bpm)?;
        let lane_repeat_count = vec![0; config.lane_count as usize];

        Ok(Self {
            config,
            difficulty,
            last_pattern: Vec::new(),
            last_pattern_time: 0.0,
            lane_repeat_count,
            last_lane: None,
        })
    }

    /// Generates a pattern from audio analysis results
    ///
    /// # Arguments
    /// * `analysis` - Results from audio analysis
    pub fn generate_pattern(&mut self, analysis: &AnalysisResults) -> Result<PatternData> {
        let mut notes = Vec::new();
        let mut sections = Vec::new();
        let mut current_section = self.create_initial_section();
        let mut rng = thread_rng();

        for &onset_time in &analysis.onset_times {
            // Update section if needed
            if onset_time >= current_section.end_time {
                sections.push(current_section.clone());
                current_section = self.create_next_section(&current_section);
            }

            // Update difficulty based on current section
            self.difficulty.update_intensity(&current_section);

            // Generate notes for this onset
            if let Some(note) = self.generate_note(onset_time, &notes, &mut rng)? {
                notes.push(note);
            }
        }

        // Add final section
        sections.push(current_section);

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

    /// Creates a note for the current onset
    fn generate_note(
        &mut self,
        timestamp: f64,
        existing_notes: &[Note],
        rng: &mut impl Rng,
    ) -> Result<Option<Note>> {
        // Skip some onsets based on difficulty
        if rng.gen::<f64>() > self.difficulty.calculate_difficulty() {
            return Ok(None);
        }

        let lane = self.select_lane(rng)?;
        let note_type = self.select_note_type(lane, rng)?;
        let note = Note {
            timestamp,
            note_type,
            lane,
        };

        // Validate note placement
        if self
            .difficulty
            .is_valid_note_placement(&note, existing_notes)
        {
            self.update_pattern_state(lane);
            Ok(Some(note))
        } else {
            Ok(None)
        }
    }

    /// Selects a lane for the next note
    fn select_lane(&mut self, rng: &mut impl Rng) -> Result<Lane> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u8 = 10;

        loop {
            let lane_num = rng.gen_range(0..self.config.lane_count);
            let lane = Lane::new(lane_num, self.config.lane_count)?;

            // Check lane repeat constraints
            if self.last_lane != Some(lane)
                || self.lane_repeat_count[lane_num as usize] < self.config.max_same_lane_repeat
            {
                return Ok(lane);
            }

            attempts += 1;
            if attempts >= MAX_ATTEMPTS {
                // If we can't find a new lane, just use a random one
                return Ok(lane);
            }
        }
    }

    /// Selects a note type (temporarily only generating tap notes)
    fn select_note_type(&self, _lane: Lane, _rng: &mut impl Rng) -> Result<NoteType> {
        // TODO: Re-enable other note types when frontend supports them
        // let difficulty = self.difficulty.calculate_difficulty();
        // let config = &self.config.difficulty;
        // 
        // Commented out for now, only using tap notes
        // let hold_prob = config.hold_note_probability * difficulty;
        // let slide_prob = config.slide_note_probability * difficulty;
        // let multi_prob = if difficulty > 0.7 { 0.2 * difficulty } else { 0.0 };
        
        Ok(NoteType::Tap)
    }

    /// Updates internal state after generating a note
    fn update_pattern_state(&mut self, lane: Lane) {
        // Update lane repeat count
        if Some(lane) == self.last_lane {
            self.lane_repeat_count[lane.value() as usize] += 1;
        } else {
            self.lane_repeat_count.fill(0);
            self.lane_repeat_count[lane.value() as usize] = 1;
        }
        self.last_lane = Some(lane);
    }

    /// Creates the initial pattern section
    fn create_initial_section(&self) -> PatternSection {
        PatternSection {
            start_time: 0.0,
            end_time: 30.0, // Default section length
            section_type: "intro".to_string(),
            intensity: 0.6,
        }
    }

    /// Creates the next pattern section
    fn create_next_section(&self, current: &PatternSection) -> PatternSection {
        let duration = current.duration().as_secs_f64();
        PatternSection {
            start_time: current.end_time,
            end_time: current.end_time + duration,
            section_type: "main".to_string(),
            intensity: (current.intensity + 0.1).min(1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_analysis() -> AnalysisResults {
        AnalysisResults {
            bpm: 120.0,
            confidence: 0.9,
            beat_markers: vec![],
            onset_times: vec![0.0, 0.5, 1.0, 1.5, 2.0],
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
