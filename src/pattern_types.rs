use crate::error::{Result, RhythmixError};
use serde::{Deserialize, Serialize};
use std::time::Duration;

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

    /// Get the lane number
    pub fn value(&self) -> u8 {
        self.0
    }
}

/// Different types of notes in the rhythm game
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NoteType {
    /// Basic single-tap note
    Tap,
    /// Note that must be held for a duration
    Hold {
        /// Duration to hold the note in seconds
        duration: f64,
    },
    /// Note requiring a directional swipe
    Slide {
        /// Target lane for the slide
        target_lane: Lane,
    },
    /// Multiple notes that must be hit simultaneously
    Multi {
        /// Additional lanes to hit
        additional_lanes: Vec<Lane>,
    },
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
}

impl Note {
    /// Creates a new tap note
    pub fn tap(timestamp: f64, lane: Lane) -> Self {
        Self {
            timestamp,
            note_type: NoteType::Tap,
            lane,
        }
    }

    /// Creates a new hold note
    pub fn hold(timestamp: f64, lane: Lane, duration: f64) -> Self {
        Self {
            timestamp,
            note_type: NoteType::Hold { duration },
            lane,
        }
    }

    /// Creates a new slide note
    pub fn slide(timestamp: f64, from_lane: Lane, to_lane: Lane) -> Self {
        Self {
            timestamp,
            note_type: NoteType::Slide {
                target_lane: to_lane,
            },
            lane: from_lane,
        }
    }

    /// Creates a new multi-note
    pub fn multi(timestamp: f64, main_lane: Lane, additional_lanes: Vec<Lane>) -> Self {
        Self {
            timestamp,
            note_type: NoteType::Multi { additional_lanes },
            lane: main_lane,
        }
    }

    /// Get the end time of the note (relevant for hold notes)
    pub fn end_time(&self) -> f64 {
        match &self.note_type {
            NoteType::Hold { duration } => self.timestamp + duration,
            _ => self.timestamp,
        }
    }

    /// Check if this note overlaps with another note
    pub fn overlaps_with(&self, other: &Note) -> bool {
        let (self_start, self_end) = (self.timestamp, self.end_time());
        let (other_start, other_end) = (other.timestamp, other.end_time());

        // Check if the time ranges overlap
        if self_end < other_start || other_end < self_start {
            return false;
        }

        // For multi notes, check if any lanes overlap
        match (&self.note_type, &other.note_type) {
            (
                NoteType::Multi {
                    additional_lanes: self_lanes,
                },
                NoteType::Multi {
                    additional_lanes: other_lanes,
                },
            ) => {
                let self_all_lanes: Vec<_> = std::iter::once(self.lane)
                    .chain(self_lanes.iter().cloned())
                    .collect();
                let other_all_lanes: Vec<_> = std::iter::once(other.lane)
                    .chain(other_lanes.iter().cloned())
                    .collect();

                self_all_lanes
                    .iter()
                    .any(|l1| other_all_lanes.iter().any(|l2| l1 == l2))
            }
            _ => self.lane == other.lane,
        }
    }
}

/// A pattern section with specific characteristics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSection {
    /// Start time of the section in seconds
    pub start_time: f64,
    /// End time of the section in seconds
    pub end_time: f64,
    /// Type of section (verse, chorus, etc.)
    pub section_type: String,
    /// Intensity factor affecting pattern density (0.0 to 1.0)
    pub intensity: f64,
}

impl PatternSection {
    /// Check if a timestamp falls within this section
    pub fn contains(&self, timestamp: f64) -> bool {
        timestamp >= self.start_time && timestamp < self.end_time
    }

    /// Get the duration of the section
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.end_time - self.start_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lane_creation() {
        assert!(Lane::new(0, 4).is_ok());
        assert!(Lane::new(3, 4).is_ok());
        assert!(Lane::new(4, 4).is_err());
    }

    #[test]
    fn test_note_overlap() {
        let note1 = Note::tap(1.0, Lane::new(0, 4).unwrap());
        let note2 = Note::tap(1.0, Lane::new(0, 4).unwrap());
        let note3 = Note::tap(1.0, Lane::new(1, 4).unwrap());
        let note4 = Note::hold(1.0, Lane::new(0, 4).unwrap(), 1.0);
        let note5 = Note::tap(1.5, Lane::new(0, 4).unwrap());

        assert!(note1.overlaps_with(&note2)); // Same time, same lane
        assert!(!note1.overlaps_with(&note3)); // Same time, different lane
        assert!(note4.overlaps_with(&note5)); // Hold note overlaps with tap
    }

    #[test]
    fn test_multi_note_overlap() {
        let note1 = Note::multi(
            1.0,
            Lane::new(0, 4).unwrap(),
            vec![Lane::new(1, 4).unwrap()],
        );
        let note2 = Note::multi(
            1.0,
            Lane::new(1, 4).unwrap(),
            vec![Lane::new(2, 4).unwrap()],
        );
        let note3 = Note::multi(
            1.0,
            Lane::new(3, 4).unwrap(),
            vec![Lane::new(0, 4).unwrap()],
        );

        assert!(note1.overlaps_with(&note2)); // Overlapping lanes
        assert!(note1.overlaps_with(&note3)); // Overlapping lanes through additional
    }

    #[test]
    fn test_pattern_section() {
        let section = PatternSection {
            start_time: 1.0,
            end_time: 4.0,
            section_type: "verse".to_string(),
            intensity: 0.8,
        };

        assert!(section.contains(1.0));
        assert!(section.contains(2.0));
        assert!(section.contains(3.99));
        assert!(!section.contains(0.9));
        assert!(!section.contains(4.0));

        assert_eq!(section.duration(), Duration::from_secs_f64(3.0));
    }
}
