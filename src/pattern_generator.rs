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

#[derive(Clone, Copy)]
enum PatternType {
    AlternatingEdges,
    Cascade,
    CenterFocus,
    Wave,
    CrossOver,
    Buildup,
}

impl PatternType {
    fn get_progression(intensity: f64) -> Vec<Self> {
        if intensity > 0.8 {
            vec![Self::Cascade, Self::Wave, Self::CrossOver]
        } else if intensity > 0.5 {
            vec![Self::AlternatingEdges, Self::Wave]
        } else {
            vec![Self::CenterFocus, Self::AlternatingEdges]
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

    fn get_pattern_for_section(
        section_type: &SectionType,
        intensity: f64,
        beat_number: usize,
    ) -> PatternType {
        let patterns = match section_type {
            SectionType::Intro => vec![PatternType::CenterFocus, PatternType::Buildup],
            SectionType::Verse => PatternType::get_progression(intensity),
            SectionType::Chorus => vec![PatternType::Wave, PatternType::CrossOver],
            SectionType::Bridge => vec![PatternType::Cascade, PatternType::CrossOver],
            _ => vec![PatternType::CenterFocus],
        };

        patterns[beat_number % patterns.len()]
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

    fn calculate_note_intensity(base: f64, is_subdivision: bool, beat_number: usize) -> f64 {
        let beat_position = beat_number % 4;
        let accent = match beat_position {
            0 => 1.0, // Strong downbeat
            2 => 0.8, // Medium backbeat
            _ => {
                if is_subdivision {
                    0.5
                } else {
                    0.6
                }
            } // Weaker beats/subdivisions
        };

        (base * accent).max(0.4)
    }

    fn get_lane_from_pattern(
        pattern: PatternType,
        beat_number: usize,
        is_subdivision: bool,
        last_lane: u8,
        intensity: f64,
    ) -> u8 {
        let base_lane = match pattern {
            PatternType::AlternatingEdges => {
                if beat_number % 2 == 0 {
                    0
                } else {
                    2
                }
            }
            PatternType::Cascade => (beat_number % 3) as u8,
            PatternType::CenterFocus => 1,
            PatternType::Wave => match beat_number % 4 {
                0 => 0,
                1 => 1,
                2 => 2,
                _ => 1,
            },
            PatternType::CrossOver => match beat_number % 6 {
                0 => 0,
                1 => 1,
                2 => 2,
                3 => 0,
                4 => 2,
                _ => 1,
            },
            PatternType::Buildup => match (beat_number / 4) % 3 {
                0 => 1 as u8,
                1 => (beat_number % 2 * 2) as u8,
                _ => (beat_number % 3) as u8,
            },
        };

        if is_subdivision {
            match base_lane {
                1 => {
                    if random::<bool>() {
                        0
                    } else {
                        2
                    }
                }
                _ => {
                    if intensity > 0.7 {
                        1
                    } else {
                        base_lane
                    }
                }
            }
        } else {
            base_lane
        }
    }

    pub fn generate_pattern(&mut self, analysis: &AnalysisResults) -> Result<PatternData> {
        let beat_interval = 60.0 / analysis.bpm;
        let total_duration = analysis.onset_times.last().copied().unwrap_or(0.0);

        // Generate sections first
        let sections = self.generate_sections(total_duration, analysis);

        // Create grid from onsets
        let mut note_times = analysis
            .onset_times
            .iter()
            .map(|&time| Self::snap_to_grid(time, beat_interval))
            .collect::<Vec<_>>();
        note_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        note_times.dedup_by(|a, b| (*a - *b).abs() < 0.01);

        let mut notes = Vec::new();
        let mut last_lane = 1;

        // Generate notes with patterns
        for &time in &note_times {
            let beat_pos = (time / beat_interval).floor();
            let is_subdivision = (time / beat_interval - beat_pos).abs() > 0.01;
            let beat_number = beat_pos as usize;

            // Find current section
            let (current_section, next_section) = {
                let current = sections
                    .iter()
                    .find(|s| time >= s.start_time && time < s.end_time)
                    .unwrap_or(&sections[0]);
                let next = sections
                    .iter()
                    .find(|s| time < s.start_time && s.start_time - time < beat_interval * 4.0);
                (current, next)
            };

            let pattern = if let Some(next) = next_section {
                // Transition zone - blend patterns
                let transition_progress =
                    (time - (next.start_time - beat_interval * 4.0)) / (beat_interval * 4.0);
                if random::<f64>() < transition_progress {
                    Self::get_pattern_for_section(&next.section_type, next.intensity, beat_number)
                } else {
                    Self::get_pattern_for_section(
                        &current_section.section_type,
                        current_section.intensity,
                        beat_number,
                    )
                }
            } else {
                Self::get_pattern_for_section(
                    &current_section.section_type,
                    current_section.intensity,
                    beat_number,
                )
            };

            let new_lane = Self::get_lane_from_pattern(
                pattern,
                beat_number,
                is_subdivision,
                last_lane,
                current_section.intensity,
            );

            last_lane = new_lane;
            notes.push(Note {
                timestamp: time,
                note_type: NoteType::Tap,
                lane: Lane::new(new_lane as u8, 3).unwrap(),
                intensity: Self::calculate_note_intensity(
                    current_section.intensity,
                    is_subdivision,
                    beat_number,
                ),
            });
        }

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

    fn generate_sections(
        &self,
        total_duration: f64,
        analysis: &AnalysisResults,
    ) -> Vec<PatternSection> {
        let section_duration = 16.0;
        let num_sections = (total_duration / section_duration).ceil() as usize;
        let mut sections = Vec::with_capacity(num_sections);

        let section_types = [
            SectionType::Intro,
            SectionType::Verse,
            SectionType::PreChorus,
            SectionType::Chorus,
            SectionType::Bridge,
            SectionType::Outro,
        ];

        for i in 0..num_sections {
            let start_time = i as f64 * section_duration;
            let end_time = (start_time + section_duration).min(total_duration);
            let section_idx = match i {
                0 => 0,                          // Intro
                i if i == num_sections - 1 => 5, // Outro
                i if i % 4 == 1 => 1,            // Verse
                i if i % 4 == 2 => 2,            // PreChorus
                i if i % 4 == 3 => 3,            // Chorus
                _ => 4,                          // Bridge
            };

            sections.push(PatternSection {
                start_time,
                end_time,
                section_type: section_types[section_idx].clone(),
                intensity: 0.5 + ((i as f64 % 4.0) / 8.0),
            });
        }
        sections
    }
}
