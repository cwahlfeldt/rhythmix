use crate::audio::{AnalysisConfig, AnalysisResults, AudioFeatures};
use crate::pattern::difficulty::{DifficultyConfig, DifficultyManager};
use crate::common::Result;
use crate::pattern::types::{Lane, Note, NoteType, PatternSection};
use crate::common::types::{PatternData, PatternMetadata};
use rand::random;

#[derive(Debug, Clone)]
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
}

impl PatternGenerator {
    /// Creates a new pattern generator
    pub fn new(config: GeneratorConfig, bpm: f64) -> Result<Self> {
        Ok(Self {
            difficulty: DifficultyManager::new(config.difficulty_config, bpm)?,
            bpm,
        })
    }

    /// Generate pattern from audio analysis results
    pub fn generate_pattern(&mut self, analysis: &AnalysisResults) -> Result<PatternData> {
        // Update BPM if confidence is good enough
        if analysis.tempo_confidence > 0.6 {
            let old_bpm = self.bpm;
            self.bpm = analysis.bpm;
            
            // Update difficulty when BPM changes significantly
            if (old_bpm - self.bpm).abs() > 5.0 {
                self.difficulty = DifficultyManager::new(
                    DifficultyConfig { base_level: self.difficulty.config.base_level, ..DifficultyConfig::default() },
                    self.bpm
                )?;
            }
        }

        Ok(PatternData {
            metadata: PatternMetadata {
                bpm: self.bpm,
                duration: analysis.onset_times.last().copied().unwrap_or(0.0),
                difficulty: self.difficulty.calculate_difficulty(),
                recommended_scroll_speed: self.difficulty.get_recommended_scroll_speed(),
            },
            notes: self.generate_notes_with_features(analysis)?,
            sections: self.generate_sections(analysis)?,
        })
    }

    /// Generate notes with features-based placement
    fn generate_notes_with_features(&self, analysis: &AnalysisResults) -> Result<Vec<Note>> {
        let mut notes = Vec::new();
        
        // Process each onset with its features
        for (idx, (&time, &strength)) in analysis.onset_times.iter()
            .zip(analysis.onset_strengths.iter())
            .enumerate() {
                let features = &analysis.onset_features[idx];
                let lane = self.determine_lane(features, strength);
                
                notes.push(Note {
                    timestamp: time,
                    note_type: NoteType::Tap,
                    lane: Lane::new(lane, 3)?,
                    intensity: strength as f64,
                });
        }

        // Sort by timestamp to ensure proper ordering
        notes.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap());
        
        Ok(notes)
    }

    /// Determine lane based on audio features
    fn determine_lane(&self, features: &AudioFeatures, strength: f32) -> u8 {
        // Use frequency distribution to influence lane choice:
        // - High frequencies tend toward outside lanes
        // - Bass frequencies tend toward center lane
        // - Mid frequencies spread across all lanes
        
        let high_ratio = features.high_energy / (features.bass_energy + features.mid_energy + features.high_energy).max(f32::EPSILON);
        let bass_ratio = features.bass_energy / (features.bass_energy + features.mid_energy + features.high_energy).max(f32::EPSILON);
        
        let rand_val = fastrand::f32();

        if high_ratio > 0.5 && strength > 0.7 {
            // Strong high frequencies - prefer outer lanes
            if rand_val < 0.5 { 0 } else { 2 }
        } else if bass_ratio > 0.4 {
            // Strong bass - prefer center lane
            1
        } else {
            // Balanced frequencies - distribute across lanes
            (rand_val * 3.0) as u8
        }
    }

    /// Generate sections based on musical structure
    fn generate_sections(&self, analysis: &AnalysisResults) -> Result<Vec<PatternSection>> {
        use crate::pattern::section::{SectionConfig, SectionManager};

        let total_duration = analysis.onset_times.last().copied().unwrap_or(0.0);

        // Create section manager
        let config = SectionConfig::default();
        let manager = SectionManager::new(config, total_duration);

        // Generate initial sections
        let mut sections = manager.generate_sections()?;

        // Adjust section intensities based on audio features
        if let Some(features) = &analysis.avg_features {
            for section in &mut sections {
                // Boost intensity for sections with high energy content
                let energy_boost = (features.high_energy + features.mid_energy) * 0.5;
                section.intensity = (section.intensity + energy_boost as f64).min(1.0);
            }
        }

        Ok(sections)
    }
}
