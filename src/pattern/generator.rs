use crate::audio::{AnalysisResults, AudioFeatures};
use crate::pattern::difficulty::{DifficultyConfig, DifficultyManager};
use crate::common::Result;
use crate::pattern::types::{Lane, Note, NoteType, PatternSection, SectionType};
use crate::audio::analysis::tempo::TimeSignature;
use crate::common::types::{PatternData, PatternMetadata};

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

/// Constants for human playability constraints
const MIN_NOTE_SPACING: f64 = 0.08;  // Minimum 80ms between any notes
const MIN_LANE_SPACING: f64 = 0.12; // Minimum 120ms between notes in same lane
const MAX_SEQUENTIAL_NOTES: usize = 3; // Maximum notes in sequence in same lane
const MAX_PATTERN_DENSITY: f64 = 0.9; // Maximum ratio of used grid points

pub struct PatternGenerator {
    difficulty: DifficultyManager,
    bpm: f64,
    grid_spacing: f64,      // Quarter note duration in seconds
    tolerance: f64,         // Maximum offset from grid points (in seconds)
    grid_offset: f64,       // Phase offset for grid alignment
    pattern_memory: PatternMemory,
    last_lane: Option<u8>,  // Track last lane for better patterns
    consecutive_notes: usize, // Track notes in same lane
    time_signature: TimeSignature, // Current time signature
    measure_position: usize, // Current position in measure (0 to beats_per_measure-1)
}

/// Keeps track of common rhythmic patterns
#[derive(Default)]
struct PatternMemory {
    window_size: usize,     // Size of pattern detection window
    note_history: Vec<f64>, // Recent note timings
    common_patterns: Vec<Vec<f64>>, // Detected patterns
}

impl PatternMemory {
    fn new() -> Self {
        Self {
            window_size: 8,
            note_history: Vec::new(),
            common_patterns: Vec::new(),
        }
    }

    fn add_note(&mut self, time: f64) {
        self.note_history.push(time);
        if self.note_history.len() > self.window_size {
            self.note_history.remove(0);
            self.update_patterns();
        }
    }

    fn update_patterns(&mut self) {
        if self.note_history.len() < 4 {
            return;
        }

        // Look for repeated intervals
        let mut intervals: Vec<f64> = self.note_history.windows(2)
            .map(|w| w[1] - w[0])
            .collect();

        // Normalize intervals to grid units
        let avg_interval = intervals.iter().sum::<f64>() / intervals.len() as f64;
        intervals.iter_mut().for_each(|i| *i = (*i / avg_interval).round());

        // Store pattern if it's new
        if !self.common_patterns.contains(&intervals) {
            self.common_patterns.push(intervals);
        }
    }

    fn get_pattern_suggestion(&self, base_time: f64, grid_spacing: f64) -> Option<Vec<f64>> {
        if self.common_patterns.is_empty() {
            return None;
        }

        // Select a random pattern and convert it to actual timings
        let pattern = &self.common_patterns[fastrand::usize(..self.common_patterns.len())];
        let mut times = Vec::new();
        let mut current_time = base_time;

        for &interval in pattern {
            times.push(current_time);
            current_time += interval * grid_spacing;
        }

        Some(times)
    }
}

impl PatternGenerator {
    /// Creates a new pattern generator
    pub fn new(config: GeneratorConfig, bpm: f64) -> Result<Self> {
        Ok(Self {
            difficulty: DifficultyManager::new(config.difficulty_config, bpm)?,
            bpm,
            grid_spacing: 60.0 / bpm,  // Quarter note duration
            tolerance: (60.0 / bpm) * 0.125,  // 1/8th note tolerance
            grid_offset: 0.0,
            pattern_memory: PatternMemory::new(),
            last_lane: None,
            consecutive_notes: 0,
            time_signature: TimeSignature::default(),
            measure_position: 0,
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
                // Update grid timing
                self.grid_spacing = 60.0 / self.bpm;
                self.tolerance = self.grid_spacing * 0.125;
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

    /// Generate notes with grid-aligned placement and proper measure structure
    fn generate_notes_with_features(&mut self, analysis: &AnalysisResults) -> Result<Vec<Note>> {
        let mut notes = Vec::new();
        let duration = analysis.onset_times.last().copied().unwrap_or(0.0);
        
        // Update tempo and time signature if confidence is good
        if analysis.tempo_confidence > 0.6 {
            self.bpm = analysis.bpm;
            self.grid_spacing = 60.0 / self.bpm;
            self.time_signature = analysis.time_signature.clone();
        }

        // Calculate measure length and phase
        let measure_length = self.grid_spacing * self.time_signature.beats_per_measure as f64;
        let phase_offset = if analysis.tempo_confidence > 0.6 {
            // Align to nearest measure boundary
            let first_beat = analysis.onset_times.first().copied().unwrap_or(0.0);
            first_beat % measure_length
        } else {
            0.0
        };

        // Get sections for context-aware generation
        let sections = self.generate_sections(analysis)?;
        
        // Create base rhythm notes following measure structure
        let mut time = phase_offset;
        while time < duration {
            let beat_in_measure = ((time - phase_offset) / self.grid_spacing) as usize % 
                                 self.time_signature.beats_per_measure as usize;
            let section = sections.iter()
                .find(|s| time >= s.start_time && time < s.end_time)
                .unwrap_or(&sections[0]);

            // Determine if we should add a note based on time signature and section
            let should_add_note = match self.time_signature.beats_per_measure {
                3 => { // 3/4 time
                    match section.section_type {
                        SectionType::Chorus => true, // All beats in chorus
                        SectionType::Bridge => beat_in_measure != 2, // Skip last beat
                        _ => beat_in_measure == 0, // Just downbeats
                    }
                },
                4 => { // 4/4 time
                    match section.section_type {
                        SectionType::Chorus => beat_in_measure % 2 == 0, // Strong beats
                        SectionType::Bridge => beat_in_measure == 0 || beat_in_measure == 2,
                        SectionType::Verse => beat_in_measure == 0, // Just downbeats
                        _ => beat_in_measure == 0,
                    }
                },
                6 => { // 6/8 time
                    match section.section_type {
                        SectionType::Chorus => beat_in_measure % 3 == 0, // Strong beats of compound meter
                        SectionType::Bridge => beat_in_measure == 0 || beat_in_measure == 3,
                        _ => beat_in_measure == 0,
                    }
                },
                _ => beat_in_measure == 0, // Default to downbeats only
            };

            // Add note on appropriate beats
            if should_add_note {
                let intensity = if beat_in_measure == 0 {
                    0.9 // Stronger on downbeat
                } else {
                    0.7 + (section.intensity * 0.2) // Other beats affected by section intensity
                };

                notes.push(Note {
                    timestamp: time,
                    note_type: NoteType::Tap,
                    lane: Lane::new(
                        match section.section_type {
                            SectionType::Intro => match beat_in_measure {
                                0 => 0,  // Start left
                                1 => 1,  // Then center
                                2 => 2,  // Then right
                                _ => if fastrand::f64() < 0.6 { 0 } else { 2 }, // Favor outer lanes
                            },
                            SectionType::Verse | SectionType::Verse2 => match beat_in_measure {
                                0 => if fastrand::bool() { 0 } else { 2 }, // Alternate outer on strong beats
                                1 => 1,  // Center on weak beats
                                2 => if self.last_lane == Some(0) { 2 } else { 0 }, // Alternate outer
                                _ => fastrand::u8(0..3), // Random for variety
                            },
                            SectionType::PreChorus => match beat_in_measure {
                                0 => 0,  // Build from left
                                1 => 1,  // Through center
                                2 => 2,  // To right
                                _ => fastrand::u8(0..3), // More varied
                            },
                            SectionType::Chorus | SectionType::Drop => match beat_in_measure {
                                0 => if fastrand::bool() { 0 } else { 2 }, // Strong beats on outer
                                1 => if fastrand::bool() { 0 } else { 2 }, // Keep energy high
                                2 => 1,  // Brief center
                                _ => fastrand::u8(0..3), // Full variety
                            },
                            SectionType::PostChorus => match beat_in_measure {
                                0 => 2,  // Start right
                                1 => 0,  // Quick to left
                                2 => 1,  // Return center
                                _ => if self.last_lane == Some(1) { if fastrand::bool() { 0 } else { 2 } } else { 1 },
                            },
                            SectionType::Bridge | SectionType::Breakdown => match beat_in_measure % 3 {
                                0 => 0,  // Left
                                1 => 2,  // Right
                                _ => 1,  // Center
                            },
                            SectionType::BuildUp => match beat_in_measure {
                                0 => 0,  // Left side build
                                1 => if fastrand::bool() { 1 } else { 2 }, // Increasing movement
                                2 => 2,  // Reach right
                                _ => if self.last_lane == Some(2) { 0 } else { 2 }, // Wide movements
                            },
                            _ => match beat_in_measure {  // Outro and others
                                0 => if self.last_lane == Some(2) { 0 } else { 2 }, // Alternate outer
                                1 => 1,  // Use center
                                2 => if self.last_lane == Some(0) { 2 } else { 0 }, // Keep alternating
                                _ => if fastrand::f64() < 0.6 { fastrand::u8(0..3) } else { 1 }, // Favor variety
                            },
                        },
                        3
                    )?,
                    intensity,
                });
            }

            time += self.grid_spacing;
        }

        // Process onset notes with section context
        for (idx, (&time, &strength)) in analysis.onset_times.iter()
            .zip(analysis.onset_strengths.iter())
            .enumerate() 
        {
            let section = sections.iter()
                .find(|s| time >= s.start_time && time < s.end_time)
                .unwrap_or(&sections[0]);

            // Adjust tolerance based on section type
            let section_tolerance = self.tolerance * match section.section_type {
                SectionType::Chorus => 0.8,  // Stricter in chorus
                SectionType::Bridge => 1.2,  // More lenient in bridge
                _ => 1.0,
            };

            // Calculate measure position and possible grid points
            let measure_length = self.grid_spacing * self.time_signature.beats_per_measure as f64;
            let beat_in_measure = ((time % measure_length) / self.grid_spacing) as usize;
            
            // Define grid points based on time signature and section
            let mut grid_points = match self.time_signature.beats_per_measure {
                3 => { // 3/4 time
                    vec![
                        (0.0, self.find_nearest_grid_division(time, 0.0)),  // Beat 1
                        (0.33, self.find_nearest_grid_division(time, 1.0/3.0)), // Beat 2
                        (0.67, self.find_nearest_grid_division(time, 2.0/3.0))  // Beat 3
                    ]
                },
                4 => { // 4/4 time
                    match section.section_type {
                        SectionType::Chorus | SectionType::Bridge => {
                            vec![
                                (0.0, self.find_nearest_grid_division(time, 0.0)),    // Beat 1
                                (0.25, self.find_nearest_grid_division(time, 0.25)),  // Beat 1&
                                (0.5, self.find_nearest_grid_division(time, 0.5)),    // Beat 2
                                (0.75, self.find_nearest_grid_division(time, 0.75))   // Beat 3
                            ]
                        },
                        _ => {
                            vec![
                                (0.0, self.find_nearest_grid_division(time, 0.0)),  // Beat 1
                                (0.5, self.find_nearest_grid_division(time, 0.5))   // Beat 3
                            ]
                        }
                    }
                },
                6 => { // 6/8 time
                    match section.section_type {
                        SectionType::Chorus => {
                            vec![
                                (0.0, self.find_nearest_grid_division(time, 0.0)),    // Beat 1
                                (0.33, self.find_nearest_grid_division(time, 1.0/3.0)), // Beat 3
                                (0.5, self.find_nearest_grid_division(time, 0.5)),    // Beat 4
                                (0.83, self.find_nearest_grid_division(time, 5.0/6.0))  // Beat 6
                            ]
                        },
                        _ => {
                            vec![
                                (0.0, self.find_nearest_grid_division(time, 0.0)),    // Beat 1
                                (0.5, self.find_nearest_grid_division(time, 0.5))     // Beat 4
                            ]
                        }
                    }
                },
                _ => vec![(0.0, self.find_nearest_grid_division(time, 0.0))] // Default to downbeats
            };

            // Find the closest valid grid point
            let (division, nearest_time) = grid_points.iter()
                .min_by(|(_, t1), (_, t2)| {
                    let diff1 = (time - *t1).abs();
                    let diff2 = (time - *t2).abs();
                    diff1.partial_cmp(&diff2).unwrap()
                })
                .unwrap();

            // Only add note if it's close enough to a grid point
            if (time - *nearest_time).abs() <= section_tolerance {
                let features = &analysis.onset_features[idx];
                let section = sections.iter()
                    .find(|s| *nearest_time >= s.start_time && *nearest_time < s.end_time)
                    .unwrap_or(&sections[0]);
                
                let lane = self.determine_lane(features, strength, &notes, *nearest_time, &section.section_type);
                
                // Check for existing notes and pattern playability
                let too_close = notes.iter().any(|note| 
                    (note.timestamp - *nearest_time).abs() < section_tolerance
                );

                if !too_close && self.is_pattern_playable(&notes, *nearest_time, lane) {
                    // Calculate intensity based on multiple factors
                    let grid_alignment = 1.0 - (time - *nearest_time).abs() / section_tolerance;
                    let intensity = (
                        strength as f64 * 0.4 +            // Onset strength
                        section.intensity * 0.3 +          // Section intensity
                        grid_alignment * 0.3               // Grid alignment quality
                    ).clamp(0.0, 1.0);

                    notes.push(Note {
                        timestamp: *nearest_time,
                        note_type: NoteType::Tap,
                        lane: Lane::new(lane, 3)?,
                        intensity,
                    });

                    // Update pattern memory
                    self.pattern_memory.add_note(*nearest_time);
                }
            }
        }

        // Add pattern-based notes
        if let Some(pattern_times) = self.pattern_memory.get_pattern_suggestion(0.0, self.grid_spacing) {
            for time in pattern_times {
                if time < duration && !notes.iter().any(|n| (n.timestamp - time).abs() < self.tolerance) {
                    notes.push(Note {
                        timestamp: time,
                        note_type: NoteType::Tap,
                        lane: Lane::new(fastrand::u8(0..3), 3)?,
                        intensity: 0.7,
                    });
                }
            }
        }

        // Sort by timestamp and remove duplicates
        notes.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap());
        notes.dedup_by(|a, b| (a.timestamp - b.timestamp).abs() < self.tolerance);
        
        Ok(notes)
    }

    /// Find nearest grid division based on the base grid spacing
    fn find_nearest_grid_division(&self, time: f64, division: f64) -> f64 {
        let grid_division = self.grid_spacing * division;
        let base_grid = (time / self.grid_spacing).floor() * self.grid_spacing;
        let next_grid = base_grid + self.grid_spacing;
        
        let division_time = base_grid + grid_division;
        if (time - division_time).abs() <= self.tolerance {
            division_time
        } else {
            // Check next measure's division
            let next_division = next_grid + grid_division;
            if (time - next_division).abs() <= self.tolerance {
                next_division
            } else {
                division_time // Default to current measure's division
            }
        }
    }

    /// Determine lane based on section type, audio features and playability
    fn determine_lane(&mut self, features: &AudioFeatures, strength: f32, notes: &[Note], time: f64, section_type: &SectionType) -> u8 {
                // Audio feature analysis
                let high_ratio = features.high_energy / (features.bass_energy + features.mid_energy + features.high_energy).max(f32::EPSILON);
                let bass_ratio = features.bass_energy / (features.bass_energy + features.mid_energy + features.high_energy).max(f32::EPSILON);
                let mid_ratio = features.mid_energy / (features.bass_energy + features.mid_energy + features.high_energy).max(f32::EPSILON);
                
                let mut preferred_lane = match section_type {
                    SectionType::Intro => {
                        // Intro: Start with a pattern that builds up
                        match notes.len() % 4 {
                            0 => 0,  // Start left
                            1 => if bass_ratio > 0.4 { 1 } else { 2 }, // Center or right based on bass
                            2 => 2,  // Right
                            _ => if high_ratio > 0.5 { if fastrand::bool() { 0 } else { 2 } } else { 1 } // Outer or center based on energy
                        }
                    },
                    SectionType::Verse => {
                        // Verse: Basic alternating pattern with variations
                        let pattern_pos = notes.len() % 3;
                        match pattern_pos {
                            0 => if bass_ratio > 0.5 { 1 } else { 0 },
                            1 => if high_ratio > 0.6 { 2 } else { 1 },
                            _ => if mid_ratio > 0.4 { if fastrand::bool() { 0 } else { 2 } } else { 1 }
                        }
                    },
                    SectionType::Verse2 => {
                        // Verse2: More complex variation of verse
                        if let Some(last) = self.last_lane {
                            match last {
                                0 => if high_ratio > 0.5 { 2 } else { 1 },
                                1 => if bass_ratio > 0.4 { if fastrand::bool() { 0 } else { 2 } } else { if high_ratio > 0.6 { 2 } else { 0 } },
                                _ => if mid_ratio > 0.5 { 0 } else { 1 }
                            }
                        } else {
                            fastrand::u8(0..3)
                        }
                    },
                    SectionType::PreChorus => {
                        // PreChorus: Building intensity with clear progression
                        let progress = (notes.len() % 8) as f64 / 8.0;
                        if progress < 0.3 {
                            0 // Start left
                        } else if progress < 0.6 {
                            if high_ratio > 0.5 { 2 } else { 1 } // Move right or center
                        } else {
                            if bass_ratio > 0.4 { 1 } else { fastrand::u8(0..3) } // More varied near end
                        }
                    },
                    SectionType::Chorus => {
                        // Chorus: High energy, all lanes with purpose
                        let intensity = high_ratio + bass_ratio;
                        match notes.len() % 4 {
                            0 => if intensity > 1.2 { if fastrand::bool() { 0 } else { 2 } } else { 1 },
                            1 => if high_ratio > 0.6 { 2 } else { 0 },
                            2 => if bass_ratio > 0.5 { 1 } else { if fastrand::bool() { 0 } else { 2 } },
                            _ => fastrand::u8(0..3)
                        }
                    },
                    SectionType::PostChorus => {
                        // PostChorus: Maintain energy but more structured
                        match notes.len() % 3 {
                            0 => 2, // Start right
                            1 => if bass_ratio > 0.4 { 1 } else { 0 }, // Center or left based on bass
                            _ => if high_ratio > 0.5 { if fastrand::bool() { 0 } else { 2 } } else { 1 }
                        }
                    },
                    SectionType::Bridge => {
                        // Bridge: Complex patterns with clear structure
                        let pattern_pos = notes.len() % 6;
                        match pattern_pos {
                            0 => 0, // Left
                            1 => 2, // Right
                            2 => 1, // Center
                            3 => if high_ratio > 0.5 { 2 } else { 0 }, // Dynamic choice
                            4 => if bass_ratio > 0.4 { 1 } else { if fastrand::bool() { 0 } else { 2 } },
                            _ => fastrand::u8(0..3)
                        }
                    },
                    SectionType::Breakdown => {
                        // Breakdown: Stripped back but deliberate
                        if bass_ratio > 0.6 {
                            1 // Strong bass = center
                        } else {
                            match notes.len() % 4 {
                                0 => 0,
                                1 => 2,
                                _ => if high_ratio > 0.5 { if fastrand::bool() { 0 } else { 2 } } else { 1 }
                            }
                        }
                    },
                    SectionType::BuildUp => {
                        // BuildUp: Increasing intensity and lane movement
                        let progress = (notes.len() % 12) as f64 / 12.0;
                        if progress < 0.3 {
                            if fastrand::bool() { 0 } else { 1 } // Start left/center
                        } else if progress < 0.6 {
                            if fastrand::bool() { 1 } else { 2 } // Move center/right
                        } else {
                            fastrand::u8(0..3) // Full lane usage at peak
                        }
                    },
                    SectionType::Drop => {
                        // Drop: High intensity with clear pattern
                        let pattern = notes.len() % 4;
                        match pattern {
                            0 => 0, // Left
                            1 => 2, // Right
                            2 => if bass_ratio > 0.5 { 1 } else { if fastrand::bool() { 0 } else { 2 } },
                            _ => if high_ratio > 0.6 { fastrand::u8(0..3) } else { 1 }
                        }
                    },
                    SectionType::Outro => {
                        // Outro: Winding down but still interesting
                        let remaining = notes.len() % 6;
                        match remaining {
                            0 | 3 => if bass_ratio > 0.4 { 1 } else { 0 },
                            1 | 4 => if high_ratio > 0.5 { 2 } else { 1 },
                            _ => if fastrand::bool() { 0 } else { 2 }
                        }
                    }
                };

        // Check if this would create unplayable patterns
        if let Some(last) = self.last_lane {
            if preferred_lane == last {
                // Check consecutive notes in same lane
                if self.consecutive_notes >= MAX_SEQUENTIAL_NOTES {
                    // Force lane change
                    preferred_lane = (last + 1 + fastrand::u8(0..2)) % 3;
                    self.consecutive_notes = 0;
                } else {
                    // Check timing between notes in same lane
                    if let Some(last_note) = notes.last() {
                        if time - last_note.timestamp < MIN_LANE_SPACING {
                            // Force lane change for quick notes
                            preferred_lane = (last + 1 + fastrand::u8(0..2)) % 3;
                            self.consecutive_notes = 0;
                        } else {
                            self.consecutive_notes += 1;
                        }
                    }
                }
            } else {
                self.consecutive_notes = 0;
            }
        }

        self.last_lane = Some(preferred_lane);
        preferred_lane
    }

    /// Check if adding a note would create unplayable patterns
    fn is_pattern_playable(&self, notes: &[Note], new_time: f64, new_lane: u8) -> bool {
        // Check minimum spacing between any notes
        if let Some(last_note) = notes.last() {
            if new_time - last_note.timestamp < MIN_NOTE_SPACING {
                return false;
            }
        }

        // Check density in local window
        let window_start = new_time - self.grid_spacing * 4.0; // Look at last 4 beats
        let notes_in_window = notes.iter()
            .filter(|n| n.timestamp > window_start && n.timestamp <= new_time)
            .count();
        let max_notes = (4.0 / MAX_PATTERN_DENSITY) as usize;
        if notes_in_window >= max_notes {
            return false;
        }

        // Check finger movement patterns
        let recent_notes: Vec<_> = notes.iter()
            .rev()
            .take(4)
            .collect();
        
        if recent_notes.len() >= 2 {
            // Avoid rapid back-and-forth between outer lanes
            let rapid_crossover = recent_notes.windows(2)
                .all(|w| (w[0].lane.0 == 0 && w[1].lane.0 == 2) || 
                        (w[0].lane.0 == 2 && w[1].lane.0 == 0));
            if rapid_crossover && new_time - recent_notes[0].timestamp < MIN_LANE_SPACING * 2.0 {
                return false;
            }
        }

        true
    }

    /// Generate sections based on musical structure
    fn generate_sections(&self, analysis: &AnalysisResults) -> Result<Vec<PatternSection>> {
        use crate::pattern::section::{SectionConfig, SectionManager};

        let total_duration = analysis.onset_times.last().copied().unwrap_or(0.0);

        // Create section manager
        let config = SectionConfig::default();
        let mut manager = SectionManager::new(config, total_duration);

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
