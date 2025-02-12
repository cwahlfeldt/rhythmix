use crate::common::Result;
use crate::pattern::timing::GridDivision;

/// Configuration for difficulty scaling
#[derive(Debug, Clone)]
pub struct DifficultyConfig {
    /// Base difficulty level (0.0 to 1.0)
    pub base_level: f64,
    /// Reference BPM for scaling calculations
    pub reference_bpm: f64,
}

impl Default for DifficultyConfig {
    fn default() -> Self {
        Self {
            base_level: 0.5,
            reference_bpm: 120.0,
        }
    }
}

/// Manages difficulty scaling based on BPM and section intensity
pub struct DifficultyManager {
    pub config: DifficultyConfig,
    bpm: f64,
    current_intensity: f64,
    grid_division: GridDivision,
}

impl DifficultyManager {
    /// Creates a new DifficultyManager with the given configuration and BPM
    pub fn new(config: DifficultyConfig, bpm: f64) -> Result<Self> {
        let manager = Self {
            config: config.clone(),
            bpm,
            current_intensity: config.base_level,
            grid_division: GridDivision::Quarter,
        };
        Ok(manager)
    }

    /// Update the grid division setting
    pub fn set_grid_division(&mut self, division: GridDivision) {
        self.grid_division = division;
    }

    /// Gets the recommended scroll speed based on current difficulty
    pub fn get_recommended_scroll_speed(&self) -> f64 {
        let base_speed = 2.0;
        let difficulty = self.config.base_level;
        let bpm_factor = (self.bpm / self.config.reference_bpm).clamp(0.5, 2.0);

        base_speed * (1.0 + difficulty * 0.5) * bpm_factor
    }

    /// Calculates the current difficulty rating (0.0 to 1.0)
    pub fn calculate_difficulty(&self) -> f64 {
        let intensity_factor = self.current_intensity;
        let bpm_factor = (self.bpm / self.config.reference_bpm).clamp(0.5, 2.0);
        
        // Calculate division difficulty factor
        let division_factor = match self.grid_division {
            GridDivision::DoubleBreve => 0.3,  // Very Easy
            GridDivision::Breve => 0.4,        // Easy
            GridDivision::Whole => 0.6,        // Moderate Easy
            GridDivision::Half => 0.8,         // Moderate
            GridDivision::Quarter => 1.0,      // Base difficulty
            GridDivision::Eighth => 1.3,       // More challenging
            GridDivision::Sixteenth => 1.6,    // Most challenging
        };

        // Combine factors with base difficulty
        (self.config.base_level * bpm_factor * intensity_factor * division_factor).clamp(0.0, 1.0)
    }
}
