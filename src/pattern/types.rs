use crate::common::{Result, RhythmixError};
use serde::{Deserialize, Serialize};

/// Represents a lane in the rhythm game (0-3 for a 4-lane layout)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lane(pub u8);

impl Lane {
    /// Creates a new Lane
    ///
    /// # Arguments
    /// * `lane` - Lane number (0-3)
    ///
    /// # Errors
    /// Returns an error if the lane number is invalid
    pub fn new(lane: u8, max_lanes: u8) -> Result<Self> {
        if lane >= max_lanes {
            return Err(RhythmixError::InvalidConfig(format!(
                "Lane {} is invalid for {}-lane layout",
                lane, max_lanes
            )));
        }
        Ok(Self(lane))
    }
}

/// Type of note in the rhythm game
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NoteType {
    /// Basic single-tap note
    Tap,
}

/// A note in the rhythm game pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// When the note should be hit (in seconds)
    pub timestamp: f64,
    /// Type of note
    #[serde(flatten)]
    pub note_type: NoteType,
    /// Lane position
    pub lane: Lane,
    /// Note intensity for visual effects (0.0 to 1.0)
    #[serde(default = "default_intensity")]
    pub intensity: f64,
}

fn default_intensity() -> f64 {
    0.8
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone, Copy)]
pub enum SectionType {
    Intro,     // Builds up intensity, center focused
    Verse,     // Main song section, alternating edges
    Chorus,    // High intensity wave pattern
    Bridge,    // Complex cascading pattern
    PreChorus, // Build up to chorus
    Outro,     // Wind down, center focused
}

/// A pattern section with specific characteristics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSection {
    /// Start time of the section in seconds
    pub start_time: f64,
    /// End time of the section in seconds
    pub end_time: f64,
    /// Type of section (verse, chorus, etc.)
    pub section_type: SectionType,
    /// Intensity factor affecting pattern density (0.0 to 1.0)
    pub intensity: f64,
}

impl PatternSection {
    /// Creates a new pattern section
    pub fn new(start_time: f64, end_time: f64, section_type: SectionType, intensity: f64) -> Self {
        Self {
            start_time,
            end_time,
            section_type,
            intensity,
        }
    }

    /// Get the duration of the section in seconds
    #[allow(dead_code)]
    pub fn duration(&self) -> f64 {
        self.end_time - self.start_time
    }

    /// Check if a timestamp falls within this section
    #[allow(dead_code)]
    pub fn contains(&self, timestamp: f64) -> bool {
        timestamp >= self.start_time && timestamp < self.end_time
    }

    /// Get the relative position within the section (0.0 to 1.0)
    #[allow(dead_code)]
    pub fn relative_position(&self, timestamp: f64) -> Option<f64> {
        if self.contains(timestamp) {
            Some((timestamp - self.start_time) / self.duration())
        } else {
            None
        }
    }

    /// Get the recommended note density based on section type and intensity
    #[allow(dead_code)]
    pub fn get_note_density(&self) -> f64 {
        let base_density = match self.section_type {
            SectionType::Intro => 0.5,
            SectionType::Verse => 0.7,
            SectionType::PreChorus => 0.8,
            SectionType::Chorus => 1.0,
            SectionType::Bridge => 0.9,
            SectionType::Outro => 0.6,
        };

        base_density * self.intensity
    }

    /// Merge with another section if they are adjacent and of the same type
    #[allow(dead_code)]
    pub fn try_merge(&self, other: &Self) -> Option<Self> {
        if self.section_type == other.section_type
            && (self.end_time - other.start_time).abs() < f64::EPSILON
        {
            Some(Self::new(
                self.start_time,
                other.end_time,
                self.section_type.clone(),
                (self.intensity + other.intensity) / 2.0,
            ))
        } else {
            None
        }
    }
}

impl PartialOrd for PatternSection {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.start_time.partial_cmp(&other.start_time)?)
    }
}

impl Ord for PatternSection {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.start_time
            .partial_cmp(&other.start_time)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl Eq for PatternSection {}

impl PartialEq for PatternSection {
    fn eq(&self, other: &Self) -> bool {
        self.start_time == other.start_time
            && self.end_time == other.end_time
            && self.section_type == other.section_type
    }
}
