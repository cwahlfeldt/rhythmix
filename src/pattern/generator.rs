use crate::audio::analysis::tempo::TimeSignature;
use crate::audio::AnalysisResults;
use crate::common::types::{PatternData, PatternMetadata};
use crate::common::Result;
use crate::pattern::difficulty::{DifficultyConfig, DifficultyManager};
use crate::pattern::timing::{BeatGrid, GridDivision};
use crate::pattern::types::{Lane, Note, NoteType};

pub struct GeneratorConfig {
    pub difficulty_config: DifficultyConfig,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            difficulty_config: DifficultyConfig::default(),
        }
    }
}

pub struct PatternGenerator {
    difficulty: DifficultyManager,
    bpm: f64,
    grid: BeatGrid,
    last_lane: Option<u8>,
    consecutive_notes: usize,
    time_signature: TimeSignature,
    song_name: String,
    grid_division: GridDivision,
}

impl PatternGenerator {
    pub fn new(config: GeneratorConfig, bpm: f64, song_name: String) -> Result<Self> {
        let mut grid = BeatGrid::new(bpm);
        let grid_division = GridDivision::Quarter; // Default to quarter notes
        grid.set_grid_division(grid_division);
        
        let mut difficulty = DifficultyManager::new(config.difficulty_config, bpm)?;
        difficulty.set_grid_division(grid_division);

        Ok(Self {
            difficulty,
            bpm,
            grid,
            last_lane: None,
            consecutive_notes: 0,
            time_signature: TimeSignature::default(),
            song_name,
            grid_division,
        })
    }

    pub fn set_grid_division(&mut self, division: GridDivision) {
        self.grid_division = division;
        self.grid.set_grid_division(division);
        self.difficulty.set_grid_division(division);
    }

    pub fn get_grid_division(&self) -> GridDivision {
        self.grid_division
    }

    pub fn generate_pattern(&mut self, analysis: &AnalysisResults) -> Result<PatternData> {
        if analysis.tempo_confidence > 0.6 {
            self.bpm = analysis.bpm;
            self.grid = BeatGrid::new(self.bpm);
            self.grid.set_grid_division(self.grid_division);
            log::info!("Using detected BPM: {:.1}", self.bpm);
        }

        let notes = self.generate_notes_with_features(analysis)?;

        Ok(PatternData {
            metadata: PatternMetadata {
                bpm: self.bpm,
                duration: analysis.onset_times.last().copied().unwrap_or(0.0),
                difficulty: self.difficulty.calculate_difficulty(),
                recommended_scroll_speed: self.difficulty.get_recommended_scroll_speed(),
                name: self.song_name.clone(),
            },
            notes,
            sections: Vec::new(),
        })
    }

    fn generate_notes_with_features(&mut self, analysis: &AnalysisResults) -> Result<Vec<Note>> {
        let mut notes = Vec::new();
        let duration = analysis.onset_times.last().copied().unwrap_or(0.0);
        
        // Calculate exact beat duration
        let beat_duration = 60.0 / self.bpm;
        
        // Calculate total number of beats
        let total_beats = (duration / beat_duration).ceil() as i32;
        
        // Generate a note for each beat in 4/4 time
        for beat in 0..total_beats {
            // Calculate exact beat timestamp
            let timestamp = beat as f64 * beat_duration;
            
            // Skip if we've exceeded the duration
            if timestamp >= duration {
                break;
            }
            
            // In 4/4 time, determine which beat in the measure (0-3)
            let beat_in_measure = beat % 4;
            
            // Assign lane based on beat position in measure
            let lane = match beat_in_measure {
                0 => 0,  // First beat: Left lane
                1 => 1,  // Second beat: Center lane
                2 => 2,  // Third beat: Right lane
                3 => 1,  // Fourth beat: Center lane
                _ => unreachable!(),
            };
            
            notes.push(Note {
                timestamp,
                note_type: NoteType::Tap,
                lane: Lane::new(lane, 3)?,
                intensity: 1.0,
            });
        }
        
        Ok(notes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_approx_eq::assert_approx_eq;

    #[test]
    fn test_quarter_note_spacing() {
        let config = GeneratorConfig::default();
        let mut generator = PatternGenerator::new(config, 120.0, "test".to_string()).unwrap();
        
        // Create test analysis data
        let analysis = AnalysisResults {
            bpm: 120.0,
            tempo_confidence: 1.0,
            time_signature: TimeSignature::default(),
            onset_times: vec![0.0, 0.5, 1.0, 1.5, 2.0],  // 2 seconds of data
            onset_strengths: vec![1.0, 1.0, 1.0, 1.0, 1.0],
            onset_features: vec![],
            avg_features: None,
        };
        
        let pattern = generator.generate_pattern(&analysis).unwrap();
        let notes = pattern.notes;
        
        // At 120 BPM, quarter notes should be exactly 0.5 seconds apart
        for notes in notes.windows(2) {
            assert_approx_eq!(notes[1].timestamp - notes[0].timestamp, 0.5);
        }
        
        // Pattern should follow left-center-right-center
        assert_eq!(notes[0].lane.0, 0); // First beat: left
        assert_eq!(notes[1].lane.0, 1); // Second beat: center
        assert_eq!(notes[2].lane.0, 2); // Third beat: right
        assert_eq!(notes[3].lane.0, 1); // Fourth beat: center
    }

    #[test]
    fn test_exact_beat_timing() {
        let config = GeneratorConfig::default();
        let bpm = 174.0;
        let mut generator = PatternGenerator::new(config, bpm, "test".to_string()).unwrap();
        
        let analysis = AnalysisResults {
            bpm,
            tempo_confidence: 1.0,
            time_signature: TimeSignature::default(),
            onset_times: vec![0.0, 1.0, 2.0],  // Doesn't matter, we ignore onsets now
            onset_strengths: vec![1.0, 1.0, 1.0],
            onset_features: vec![],
            avg_features: None,
        };
        
        let pattern = generator.generate_pattern(&analysis).unwrap();
        let notes = pattern.notes;
        
        // Calculate exact beat duration
        let beat_duration = 60.0 / bpm;
        
        // Check first measure (4 beats)
        for i in 0..4 {
            let expected_time = i as f64 * beat_duration;
            assert_approx_eq!(notes[i].timestamp, expected_time, 0.000001);
            
            // Verify lane pattern (Left -> Center -> Right -> Center)
            let expected_lane = match i % 4 {
                0 => 0,  // Left
                1 => 1,  // Center
                2 => 2,  // Right
                3 => 1,  // Center
                _ => unreachable!(),
            };
            assert_eq!(notes[i].lane.0, expected_lane);
        }
    }

    #[test]
    fn test_different_bpm() {
        let config = GeneratorConfig::default();
        let mut generator = PatternGenerator::new(config, 140.0, "test".to_string()).unwrap();
        
        let analysis = AnalysisResults {
            bpm: 140.0,
            tempo_confidence: 1.0,
            time_signature: TimeSignature::default(),
            onset_times: vec![0.0, 0.42857, 0.85714, 1.28571],  // Approx. 140 BPM timings
            onset_strengths: vec![1.0, 1.0, 1.0, 1.0],
            onset_features: vec![],
            avg_features: None,
        };
        
        let pattern = generator.generate_pattern(&analysis).unwrap();
        let notes = pattern.notes;
        
        // At 140 BPM, quarter notes should be exactly 0.42857... seconds apart
        let expected_spacing = 60.0 / 140.0;
        for notes in notes.windows(2) {
            assert_approx_eq!(notes[1].timestamp - notes[0].timestamp, expected_spacing, 0.0001);
        }
    }
}