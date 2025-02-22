use crate::common::Result;

/// Core timing constants for 4/4 time
pub const BEATS_PER_MEASURE: usize = 4; // 4/4 time signature
pub const SUBDIVISION_TOLERANCE: f64 = 0.00002; // 20ms snap tolerance

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridDivision {
    DoubleBreve = 1,    // Slowest - one note every 8 beats
    Breve = 2,          // One note every 4 beats
    Whole = 4,          // One note every 2 beats
    Half = 8,           // One note per beat
    Quarter = 16,       // Two notes per beat (default)
    Eighth = 32,        // Four notes per beat
    Sixteenth = 64,     // Eight notes per beat
}

impl Default for GridDivision {
    fn default() -> Self {
        GridDivision::Quarter
    }
}

/// Handles precise grid timing calculations
pub struct BeatGrid {
    bpm: f64,
    seconds_per_beat: f64,
    grid_division: GridDivision,
    seconds_per_division: f64,
}

impl BeatGrid {
    /// Create a new beat grid with the given BPM
    pub fn new(bpm: f64) -> Self {
        let seconds_per_beat = 60.0 / bpm;
        let grid_division = GridDivision::default();
        let seconds_per_division = Self::calculate_division_duration(seconds_per_beat, grid_division);

        Self {
            bpm,
            seconds_per_beat,
            grid_division,
            seconds_per_division,
        }
    }

    fn calculate_division_duration(seconds_per_beat: f64, division: GridDivision) -> f64 {
        match division {
            GridDivision::DoubleBreve => seconds_per_beat * 8.0,   // 8 beats
            GridDivision::Breve => seconds_per_beat * 4.0,         // 4 beats
            GridDivision::Whole => seconds_per_beat * 2.0,         // 2 beats
            GridDivision::Half => seconds_per_beat,                // 1 beat
            GridDivision::Quarter => seconds_per_beat * 0.5,       // 1/2 beat
            GridDivision::Eighth => seconds_per_beat * 0.25,       // 1/4 beat
            GridDivision::Sixteenth => seconds_per_beat * 0.125,   // 1/8 beat
        }
    }

    /// Set the grid division type
    pub fn set_grid_division(&mut self, division: GridDivision) {
        self.grid_division = division;
        self.seconds_per_division = Self::calculate_division_duration(self.seconds_per_beat, division);
    }

    /// Get the current grid division type
    pub fn get_grid_division(&self) -> GridDivision {
        self.grid_division
    }

    /// Calculate the nearest grid position for a given time
    pub fn snap_to_grid(&self, time: f64) -> f64 {
        let division_index = (time / self.seconds_per_division).round() as i64;
        division_index as f64 * self.seconds_per_division
    }

    /// Check if a time is close enough to a grid position
    pub fn is_on_grid(&self, time: f64) -> bool {
        let nearest_grid = self.snap_to_grid(time);
        (time - nearest_grid).abs() <= SUBDIVISION_TOLERANCE
    }

    /// Get the grid position indices for a time range
    pub fn get_grid_positions(&self, start_time: f64, end_time: f64) -> Vec<f64> {
        let start_division = (start_time / self.seconds_per_division).ceil() as i64;
        let end_division = (end_time / self.seconds_per_division).floor() as i64;

        (start_division..=end_division)
            .map(|division| division as f64 * self.seconds_per_division)
            .collect()
    }

    /// Calculate which beat in the measure a time falls on (0-based)
    pub fn get_beat_in_measure(&self, time: f64) -> usize {
        let beats = (time / self.seconds_per_beat).floor();
        (beats as usize) % BEATS_PER_MEASURE
    }

    /// Calculate which subdivision within a beat a time falls on (0-based)
    pub fn get_subdivision_in_beat(&self, time: f64) -> usize {
        let beat_time = time % self.seconds_per_beat;
        let subdivision = (beat_time / self.seconds_per_division).round() as usize;
        subdivision % self.grid_division as usize
    }

    /// Convert between beats and seconds
    pub fn beats_to_seconds(&self, beats: f64) -> f64 {
        beats * self.seconds_per_beat
    }

    /// Convert between seconds and beats
    pub fn seconds_to_beats(&self, seconds: f64) -> f64 {
        seconds / self.seconds_per_beat
    }

    /// Get the duration of one beat in seconds
    pub fn get_seconds_per_beat(&self) -> f64 {
        self.seconds_per_beat
    }

    /// Get the duration of one grid division in seconds
    pub fn get_seconds_per_division(&self) -> f64 {
        self.seconds_per_division
    }

    /// Calculate BPM from inter-onset intervals
    pub fn calculate_bpm_from_intervals(intervals: &[f64]) -> Option<f64> {
        if intervals.is_empty() {
            return None;
        }

        // Calculate average interval
        let avg_interval = intervals.iter().sum::<f64>() / intervals.len() as f64;

        // Convert to BPM
        Some(60.0 / avg_interval)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use assert_approx_eq::assert_approx_eq;

//     #[test]
//     fn test_grid_snapping() {
//         let grid = BeatGrid::new(120.0); // 120 BPM = 0.5s per beat

//         // Test exact grid points
//         assert_approx_eq!(grid.snap_to_grid(0.5), 0.5); // On beat
//         assert_approx_eq!(grid.snap_to_grid(0.25), 0.25); // On 8th note

//         // Test snapping to nearest grid point
//         assert_approx_eq!(grid.snap_to_grid(0.51), 0.5); // Should snap to beat
//         assert_approx_eq!(grid.snap_to_grid(0.24), 0.25); // Should snap to 8th note
//     }

//     #[test]
//     fn test_grid_positions() {
//         let grid = BeatGrid::new(120.0);
//         let positions = grid.get_grid_positions(0.0, 1.0);

//         // At 120 BPM with 16 divisions per beat, we should get 32 positions per second
//         assert_eq!(positions.len(), 33); // Include both start and end

//         // First few positions should be at exact subdivisions
//         assert_approx_eq!(positions[0], 0.0);
//         assert_approx_eq!(positions[1], 0.125);
//         assert_approx_eq!(positions[2], 0.25);
//     }

//     #[test]
//     fn test_beat_and_subdivision() {
//         let grid = BeatGrid::new(120.0);

//         // Test beat positions
//         assert_eq!(grid.get_beat_in_measure(0.0), 0);
//         assert_eq!(grid.get_beat_in_measure(0.5), 1);
//         assert_eq!(grid.get_beat_in_measure(1.0), 2);

//         // Test subdivisions
//         assert_eq!(grid.get_subdivision_in_beat(0.0), 0);
//         assert_eq!(grid.get_subdivision_in_beat(0.125), 4);
//         assert_eq!(grid.get_subdivision_in_beat(0.25), 8);
//     }

//     #[test]
//     fn test_bpm_calculation() {
//         let intervals = vec![0.5, 0.5, 0.5, 0.5]; // 120 BPM intervals
//         let calculated_bpm = BeatGrid::calculate_bpm_from_intervals(&intervals).unwrap();
//         assert_approx_eq!(calculated_bpm, 120.0);
//     }
// }
