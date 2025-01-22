use crate::error::{Result, RhythmixError};
use crate::pattern_types::{Note, PatternSection};
use std::time::Duration;

/// Configuration for difficulty scaling
#[derive(Debug, Clone)]
pub struct DifficultyConfig {
    /// Base difficulty level (0.0 to 1.0)
    pub base_level: f64,
    /// Minimum time between notes in seconds
    pub min_note_interval: f64,
    /// Maximum time between notes in seconds
    pub max_note_interval: f64,
    /// Reference BPM for scaling calculations
    pub reference_bpm: f64,
}

impl Default for DifficultyConfig {
    fn default() -> Self {
        Self {
            base_level: 0.5,
            min_note_interval: 0.2,
            max_note_interval: 2.0,
            reference_bpm: 120.0,
        }
    }
}

/// Handles difficulty calculations and pattern scaling
pub struct DifficultyManager {
    config: DifficultyConfig,
    bpm: f64,
    current_intensity: f64,
}

impl DifficultyManager {
    /// Creates a new DifficultyManager
    ///
    /// # Arguments
    /// * `config` - Difficulty configuration
    /// * `bpm` - Detected BPM of the audio
    ///
    /// # Errors
    /// Returns an error if the configuration is invalid
    pub fn new(config: DifficultyConfig, bpm: f64) -> Result<Self> {
        Self::validate_config(&config)?;

        Ok(Self {
            config,
            bpm,
            current_intensity: 1.0,
        })
    }

    /// Validates the difficulty configuration
    fn validate_config(config: &DifficultyConfig) -> Result<()> {
        if !(0.0..=1.0).contains(&config.base_level) {
            return Err(RhythmixError::InvalidConfig(
                "Base difficulty level must be between 0.0 and 1.0".into(),
            ));
        }

        if config.min_note_interval <= 0.0 || config.max_note_interval <= 0.0 {
            return Err(RhythmixError::InvalidConfig(
                "Note intervals must be positive".into(),
            ));
        }

        if config.min_note_interval >= config.max_note_interval {
            return Err(RhythmixError::InvalidConfig(
                "Minimum note interval must be less than maximum".into(),
            ));
        }

        if config.reference_bpm <= 0.0 {
            return Err(RhythmixError::InvalidConfig(
                "Reference BPM must be positive".into(),
            ));
        }

        Ok(())
    }

    /// Updates the current intensity based on a pattern section
    pub fn update_intensity(&mut self, section: &PatternSection) {
        self.current_intensity = section.intensity;
    }

    /// Calculates the current difficulty rating (0.0 to 1.0)
    pub fn calculate_difficulty(&self) -> f64 {
        let bpm_factor = self.calculate_bpm_factor();
        let intensity_factor = self.current_intensity;

        // Combine factors with base difficulty
        (self.config.base_level * bpm_factor * intensity_factor).clamp(0.0, 1.0)
    }

    /// Calculates difficulty factor based on BPM
    fn calculate_bpm_factor(&self) -> f64 {
        let bpm_ratio = self.bpm / self.config.reference_bpm;
        // Scale difficulty non-linearly with BPM
        (bpm_ratio * 0.8 + 0.2).clamp(0.5, 2.0)
    }

    /// Gets the current minimum time between notes
    pub fn get_min_note_interval(&self) -> Duration {
        let base_interval = self.config.min_note_interval;
        let difficulty_factor = 1.0 + (1.0 - self.calculate_difficulty()) * 0.5;
        Duration::from_secs_f64(base_interval * difficulty_factor)
    }

    /// Gets the current maximum time between notes
    pub fn get_max_note_interval(&self) -> Duration {
        let base_interval = self.config.max_note_interval;
        let difficulty_factor = 1.0 - self.calculate_difficulty() * 0.3;
        Duration::from_secs_f64(base_interval * difficulty_factor)
    }

    /// Determines if a note placement is valid based on current difficulty
    pub fn is_valid_note_placement(&self, new_note: &Note, existing_notes: &[Note]) -> bool {
        let min_interval = self.get_min_note_interval().as_secs_f64();

        // Check minimum time interval between notes
        for note in existing_notes {
            let time_diff = (new_note.timestamp - note.timestamp).abs();
            if time_diff < min_interval && new_note.overlaps_with(note) {
                return false;
            }
        }

        true
    }

    /// Gets the recommended scroll speed based on current difficulty
    pub fn get_recommended_scroll_speed(&self) -> f64 {
        let base_speed = 2.0;
        let difficulty = self.calculate_difficulty();
        let bpm_factor = (self.bpm / self.config.reference_bpm).clamp(0.5, 2.0);

        base_speed * (1.0 + difficulty * 0.5) * bpm_factor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern_types::Lane;

    #[test]
    fn test_difficulty_calculation() {
        let config = DifficultyConfig::default();
        let manager = DifficultyManager::new(config.clone(), 120.0).unwrap();

        let difficulty = manager.calculate_difficulty();
        assert!((0.0..=1.0).contains(&difficulty));

        // Test with higher BPM
        let manager_fast = DifficultyManager::new(config.clone(), 180.0).unwrap();
        let difficulty_fast = manager_fast.calculate_difficulty();
        assert!(difficulty_fast > difficulty);
    }

    #[test]
    fn test_note_placement_validation() {
        let config = DifficultyConfig::default();
        let manager = DifficultyManager::new(config, 120.0).unwrap();

        let note1 = Note::tap(1.0, Lane::new(0, 4).unwrap());
        let note2 = Note::tap(1.1, Lane::new(0, 4).unwrap());
        let note3 = Note::tap(2.0, Lane::new(1, 4).unwrap());

        assert!(manager.is_valid_note_placement(&note3, &[note1.clone()]));
        assert!(!manager.is_valid_note_placement(&note2, &[note1]));
    }

    #[test]
    fn test_scroll_speed_calculation() {
        let config = DifficultyConfig::default();
        let manager = DifficultyManager::new(config.clone(), 120.0).unwrap();
        let base_speed = manager.get_recommended_scroll_speed();

        let manager_fast = DifficultyManager::new(config, 180.0).unwrap();
        let fast_speed = manager_fast.get_recommended_scroll_speed();

        assert!(fast_speed > base_speed);
    }

    #[test]
    fn test_invalid_config() {
        let mut config = DifficultyConfig::default();
        config.base_level = 1.5;
        assert!(DifficultyManager::new(config, 120.0).is_err());
    }
}
