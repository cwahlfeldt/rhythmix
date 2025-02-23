use crate::pattern::types::{Note, PatternSection as Section};
use serde::{Deserialize, Serialize};

/// Metadata about the analyzed audio and generated pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMetadata {
    /// Detected beats per minute (high precision)
    pub bpm: f64,
    /// Duration of the audio in seconds
    pub duration: f64,
    /// Calculated difficulty rating (0.0 to 1.0)
    pub difficulty: f64,
    /// Recommended scroll speed for optimal gameplay
    pub recommended_scroll_speed: f64,
    /// Name of the song
    pub name: String,
    /// Grid division setting used
    pub grid_division: String,
    /// Seconds per beat (high precision)
    pub seconds_per_beat: f64,
    /// Seconds per grid division (high precision)
    pub seconds_per_division: f64,
}

/// Beat marker for visual feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatMarker {
    /// Timestamp of the beat in seconds
    pub timestamp: f64,
    /// Whether this is the first beat of a measure
    pub is_strong_beat: bool,
}

/// Complete pattern data including metadata and all game elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternData {
    pub metadata: PatternMetadata,
    pub notes: Vec<Note>,
    pub sections: Vec<Section>,
}
