use crate::audio_analyzer::AnalysisResults;
use crate::difficulty::{DifficultyConfig, DifficultyManager};
use crate::error::Result;
use crate::pattern_types::{Lane, Note, NoteType, PatternSection, SectionType};
use crate::types::{PatternData, PatternMetadata};
use rand::random;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct GeneratorConfig {
    pub lane_count: u8,
    pub difficulty: DifficultyConfig,
    pub min_pattern_repeat_time: Duration,
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

pub struct PatternGenerator {
    config: GeneratorConfig,
    difficulty: DifficultyManager,
}

impl PatternGenerator {
    pub fn new(config: GeneratorConfig, bpm: f64) -> Result<Self> {
        let difficulty = DifficultyManager::new(config.difficulty.clone(), bpm)?;
        Ok(Self { config, difficulty })
    }

    fn get_note_intensity(time: f64, beat_interval: f64, onset_strength: Option<f64>) -> f64 {
        let beat_position = time / beat_interval;
        let position_in_beat = beat_position.fract();
        let bar_position = beat_position.floor() as usize % 4;

        let base_intensity = match (bar_position, position_in_beat) {
            (0, x) if x < 0.01 => 1.0,    // Bar start
            (2, x) if x < 0.01 => 0.9,    // Third beat
            (_, x) if x < 0.01 => 0.8,    // Main beat
            (_, x) if (x - 0.5).abs() < 0.01 => 0.7,  // Eighth note
            (_, x) if (x - 0.25).abs() < 0.01 || (x - 0.75).abs() < 0.01 => 0.6,  // Sixteenth note
            _ => 0.5,
        };

        if let Some(strength) = onset_strength {
            (base_intensity + strength) / 2.0
        } else {
            base_intensity
        }
    }

    fn snap_to_grid(time: f64, beat_interval: f64) -> f64 {
        let beat_position = time / beat_interval;
        let beat_number = beat_position.floor();
        let position_in_beat = beat_position.fract();

        let snapped_position = match position_in_beat {
            p if p < 0.125 => 0.0,
            p if p < 0.375 => 0.25,
            p if p < 0.625 => 0.5,
            p if p < 0.875 => 0.75,
            _ => 1.0,
        };

        (beat_number + snapped_position) * beat_interval
    }

    pub fn generate_pattern(&mut self, analysis: &AnalysisResults) -> Result<PatternData> {
        log::info!("Generating pattern with BPM: {}", analysis.bpm);

        let beat_interval = 60.0 / analysis.bpm;
        let total_duration = analysis.onset_times.last().copied().unwrap_or(0.0);

        // First: Snap all onsets to grid positions
        let mut snapped_times = analysis
            .onset_times
            .iter()
            .map(|&time| Self::snap_to_grid(time, beat_interval))
            .collect::<Vec<_>>();

        // Remove duplicates
        snapped_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        snapped_times.dedup_by(|a, b| (*a - *b).abs() < 0.01);

        // Generate notes
        let mut notes = Vec::new();
        let mut last_lane = 1;

        for &time in &snapped_times {
            let beat_position = time / beat_interval;
            let full_beat = (beat_position.round() - beat_position).abs() < 0.01;
            let beat_number = beat_position.round() as usize;

            let lane = if full_beat {
                // Main beats follow standard pattern
                match beat_number % 4 {
                    0 => 1, // Downbeat in center
                    2 => {
                        if random::<bool>() {
                            1
                        } else {
                            if last_lane == 0 {
                                2
                            } else {
                                0
                            }
                        }
                    }
                    _ => {
                        let mut new_lane;
                        loop {
                            new_lane = random::<usize>() % 3;
                            if i32::abs(new_lane as i32 - last_lane as i32) <= 1 {
                                break;
                            }
                        }
                        new_lane
                    }
                }
            } else {
                // Subdivisions alternate between outer lanes based on last_lane
                if last_lane == 1 {
                    if random::<bool>() {
                        0
                    } else {
                        2
                    }
                } else {
                    last_lane // Keep same lane for fast sequences
                }
            };

            last_lane = lane;
            let base_intensity = Self::get_note_intensity(time, beat_interval, None);
            let new_lane = if base_intensity > 0.8 {
                1  // Important beats in center
            } else if base_intensity > 0.6 {
                if last_lane == 1 { if random::<bool>() { 0 } else { 2 } } else { 1 }
            } else {
                // Keep pattern for weaker beats
                if last_lane == 1 {
                    if random::<bool>() { 0 } else { 2 }
                } else {
                    last_lane
                }
            };

            last_lane = new_lane;
            notes.push(Note {
                timestamp: time,
                note_type: NoteType::Tap,
                lane: Lane::new(new_lane as u8, 3).unwrap(),
                intensity: base_intensity,
            });
        }

        // Generate sections
        let total_beats = (total_duration / beat_interval).ceil() as usize;
        let section_duration = 16.0;
        let num_sections = (total_duration / section_duration).ceil() as usize;
        let mut sections = Vec::with_capacity(num_sections);

        let section_types = [
            SectionType::Intro,
            SectionType::Verse,
            SectionType::Chorus,
            SectionType::Bridge,
            SectionType::Outro,
        ];

        for i in 0..num_sections {
            let start_time = i as f64 * section_duration;
            let end_time = (start_time + section_duration).min(total_duration);

            let section_notes = notes
                .iter()
                .filter(|note| note.timestamp >= start_time && note.timestamp < end_time)
                .count();
            let intensity =
                (section_notes as f64 / ((end_time - start_time) / beat_interval)).min(1.0);

            sections.push(PatternSection {
                start_time,
                end_time,
                section_type: section_types[i % section_types.len()].clone(),
                intensity: 0.5 + intensity * 0.5,
            });
        }

        log::info!(
            "Generated {} notes in {} sections",
            notes.len(),
            sections.len()
        );

        Ok(PatternData {
            metadata: PatternMetadata {
                bpm: analysis.bpm,
                duration: total_duration,
                difficulty: self.difficulty.calculate_difficulty(),
                recommended_scroll_speed: self.difficulty.get_recommended_scroll_speed(),
            },
            notes,
            sections,
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
            notes: vec![],
            onset_times: vec![0.0, 0.5, 1.0, 1.5, 2.0],
            beat_times: vec![0.0, 0.5, 1.0, 1.5, 2.0],
            beat_strengths: vec![1.0, 0.5, 1.0, 0.5, 1.0],
        }
    }

    #[test]
    fn test_lane_selection() {
        let config = GeneratorConfig::default();
        let mut generator = PatternGenerator::new(config, 120.0).unwrap();
        let analysis = AnalysisResults {
            bpm: 120.0,
            confidence: 0.9,
            notes: vec![],
            onset_times: vec![
                0.0,  // Beat - center lane
                0.25, // Subdivision
                0.5,  // Beat - variable lane
                0.75, // Subdivision
                1.0,  // Beat - center lane
            ],
            beat_times: vec![0.0, 0.5, 1.0],
            beat_strengths: vec![1.0, 0.5, 1.0],
        };

        let pattern = generator.generate_pattern(&analysis).unwrap();

        // Check main beats are in appropriate lanes
        for note in &pattern.notes {
            let is_main_beat = (note.timestamp / (60.0 / 120.0)).fract().abs() < 0.01;
            if is_main_beat && (note.timestamp * 2.0).round() as usize % 2 == 0 {
                assert_eq!(note.lane.value(), 1, "Main beat should be in center lane");
            }
        }
    }

    #[test]
    fn test_snap_to_grid() {
        let generator = PatternGenerator::new(GeneratorConfig::default(), 120.0).unwrap();
        let beat_interval = 60.0 / 120.0;

        let test_times = vec![
            (0.1, 0.0),   // Should snap to 0.0
            (0.35, 0.25), // Should snap to 0.25
            (0.55, 0.5),  // Should snap to 0.5
            (0.8, 0.75),  // Should snap to 0.75
            (0.95, 1.0),  // Should snap to 1.0
        ];

        for (input, expected) in test_times {
            let snapped = PatternGenerator::snap_to_grid(input * beat_interval, beat_interval);
            assert!(
                (snapped - expected * beat_interval).abs() < 0.01,
                "Failed to snap {} to {}",
                input,
                expected
            );
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
}
