use crate::audio::{AnalysisConfig, AnalysisResults};
use crate::pattern::difficulty::{DifficultyConfig, DifficultyManager};
use crate::common::Result;
use crate::pattern::types::{Lane, Note, NoteType};
use crate::common::types::{PatternData, PatternMetadata};
use rand::random;

#[derive(Debug, Clone)]
pub struct GeneratorConfig {
    pub difficulty: DifficultyConfig,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            difficulty: DifficultyConfig::default(),
        }
    }
}

pub struct PatternGenerator {
    difficulty: DifficultyManager,
    bpm: f64,
}

impl PatternGenerator {
    /// Creates a new pattern generator
    pub fn new(config: GeneratorConfig, bpm: f64) -> Result<Self> {
        Ok(Self {
            difficulty: DifficultyManager::new(config.difficulty, bpm)?,
            bpm,
        })
    }

    /// Generate pattern from audio analysis results
    pub fn generate_pattern(&mut self, analysis: &AnalysisResults) -> Result<PatternData> {
        Ok(PatternData {
            metadata: PatternMetadata {
                bpm: self.bpm,
                duration: analysis.onset_times.last().copied().unwrap_or(0.0),
                difficulty: self.difficulty.calculate_difficulty(),
                recommended_scroll_speed: self.difficulty.get_recommended_scroll_speed(),
            },
            notes: self.generate_basic_notes(analysis)?,
            sections: Vec::new(), // Skip section generation for now
        })
    }

    fn generate_basic_notes(&self, analysis: &AnalysisResults) -> Result<Vec<Note>> {
        let mut notes = Vec::new();
        for &time in &analysis.onset_times {
            let lane = random::<u8>() % 3;
            notes.push(Note {
                timestamp: time,
                note_type: NoteType::Tap,
                lane: Lane::new(lane, 3)?,
                intensity: 1.0,
            });
        }
        Ok(notes)
    }
}
