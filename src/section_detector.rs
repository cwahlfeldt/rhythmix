use crate::error::Result;
use crate::pattern_types::{PatternSection, SectionType};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq)]
enum EnergyLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum FrequencyProfile {
    BassHeavy,   // Strong low frequencies
    MidHeavy,    // Strong mid frequencies
    HighHeavy,   // Strong high frequencies
    Balanced,    // Even distribution
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TransitionStrength {
    Weak,    // Subtle changes
    Medium,  // Noticeable changes
    Strong,  // Dramatic changes
}

const SECTION_ANALYSIS_WINDOW: usize = 4; // 4 second sliding window for energy analysis
const ENERGY_THRESHOLD_MULT: f64 = 1.2; // Multiplier for energy threshold
const MIN_SECTION_LENGTH: f64 = 4.0; // Minimum section length in seconds

/// Configuration for section detection
#[derive(Debug, Clone)]
pub struct SectionConfig {
    /// Size of the sliding window for energy analysis (in seconds)
    pub window_size: f64,
    /// Minimum section length (in seconds)
    pub min_section_length: f64,
    /// Energy threshold multiplier for section changes
    pub energy_threshold_mult: f64,
    /// Minimum section length multiplier for different section types
    pub section_length_mult: f64,
    /// Section change detection sensitivity (0.0 to 1.0)
    pub change_sensitivity: f64,
}

impl Default for SectionConfig {
    fn default() -> Self {
        Self {
            window_size: SECTION_ANALYSIS_WINDOW as f64,
            min_section_length: MIN_SECTION_LENGTH,
            energy_threshold_mult: ENERGY_THRESHOLD_MULT,
            section_length_mult: 1.5,
            change_sensitivity: 0.2, // Lower threshold for more granular detection
        }
    }
}

/// Detects song sections based on spectral energy and rhythmic patterns
pub struct SectionDetector {
    config: SectionConfig,
    energy_window: VecDeque<f64>,
    rhythm_density_window: VecDeque<f64>,
    freq_distribution_window: VecDeque<Vec<f64>>,
    current_time: f64,
    current_section: Option<PatternSection>,
    pending_sections: Vec<PatternSection>,
    last_energy: f64,
    avg_energy: f64,
    max_energy: f64,
    total_duration: f64,
}

impl SectionDetector {
    /// Creates a new SectionDetector with the specified configuration
    pub fn new(config: SectionConfig, total_duration: f64) -> Self {
        Self {
            config,
            energy_window: VecDeque::new(),
            rhythm_density_window: VecDeque::new(),
            freq_distribution_window: VecDeque::new(),
            current_time: 0.0,
            current_section: None,
            pending_sections: Vec::new(),
            last_energy: 0.0,
            avg_energy: 0.0,
            max_energy: 0.0,
            total_duration,
        }
    }

    /// Process a chunk of spectral energy and frequency data
    ///
    /// # Arguments
    /// * `energy` - Current spectral energy value
    /// * `frequencies` - Vector of frequency band energies
    /// * `rhythm_density` - Current rhythm density value (hits per second)
    /// * `time` - Current time in seconds
    pub fn process_frame(&mut self, energy: f64, frequencies: Vec<f64>, rhythm_density: f64, time: f64) -> Result<()> {
        self.current_time = time;
        self.last_energy = energy;
        
        // Update sliding windows
        self.update_windows(energy, frequencies, rhythm_density);
        self.detect_section_change();
        
        Ok(())
    }

    /// Update all analysis windows
    fn update_windows(&mut self, energy: f64, frequencies: Vec<f64>, rhythm_density: f64) {
        let window_size = (self.config.window_size * 10.0) as usize;
        
        // Update energy window
        self.energy_window.push_back(energy);
        while self.energy_window.len() > window_size {
            self.energy_window.pop_front();
        }
        
        // Update rhythm density window
        self.rhythm_density_window.push_back(rhythm_density);
        while self.rhythm_density_window.len() > window_size {
            self.rhythm_density_window.pop_front();
        }
        
        // Update frequency distribution window
        self.freq_distribution_window.push_back(frequencies);
        while self.freq_distribution_window.len() > window_size {
            self.freq_distribution_window.pop_front();
        }
        
        self.update_energy_stats();
    }

    /// Update energy statistics based on the current window
    fn update_energy_stats(&mut self) {
        if self.energy_window.is_empty() {
            return;
        }
        
        self.avg_energy = self.energy_window.iter().sum::<f64>() / self.energy_window.len() as f64;
        self.max_energy = self.energy_window.iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);
    }

    /// Get average rhythm density from the current window
    fn get_avg_rhythm_density(&self) -> f64 {
        if self.rhythm_density_window.is_empty() {
            return 0.0;
        }
        self.rhythm_density_window.iter().sum::<f64>() / self.rhythm_density_window.len() as f64
    }

    /// Calculate frequency profile similarity between current and previous windows
    fn get_freq_profile_change(&self) -> f64 {
        if self.freq_distribution_window.len() < 2 {
            return 0.0;
        }

        let current = self.freq_distribution_window.back().unwrap();
        let previous = self.freq_distribution_window.iter().rev().nth(1).unwrap();

        // Calculate Euclidean distance between frequency profiles
        current.iter()
            .zip(previous.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// Detect section changes based on multiple features
    fn detect_section_change(&mut self) {
        // Calculate multiple change indicators
        let energy_change = self.get_energy_change_strength();
        let rhythm_change = self.get_rhythm_change_strength();
        let freq_change = self.get_freq_change_strength();
        let progress = self.current_time / self.total_duration;
        
        // Analyze the strength of transition
        let transition_strength = self.analyze_transition_strength(energy_change, rhythm_change, freq_change);
        
        // Get current musical characteristics
        let energy_level = self.get_energy_level();
        let freq_profile = self.get_frequency_profile();
        
        // Determine if we should change section
        // Calculate additional change metrics
        let spectral_change = self.calculate_spectral_change();
        let rhythm_profile_change = self.calculate_rhythm_profile_change();
        
        let should_change = match &self.current_section {
            None => true,
            Some(section) => {
                let min_duration = section.duration().mul_f64(self.config.section_length_mult);
                let current_duration = section.duration();
                
                // More sensitive change detection
                current_duration >= min_duration && (
                    // Strong transitions
                    transition_strength == TransitionStrength::Strong ||
                    // Medium transitions with supporting evidence
                    (transition_strength == TransitionStrength::Medium && (
                        self.supports_section_change(energy_level, freq_profile) ||
                        spectral_change > 0.3 ||
                        rhythm_profile_change > 0.3
                    )) ||
                    // Weak transitions with multiple supporting factors
                    (transition_strength == TransitionStrength::Weak && 
                     spectral_change > 0.4 &&
                     rhythm_profile_change > 0.4) ||
                    // Time-based changes
                    current_duration >= min_duration.mul_f64(1.5) ||
                    // Dramatic spectral shifts
                    spectral_change > 0.5
                )
            }
        };

        if should_change {
            if let Some(mut section) = self.current_section.take() {
                // Handle the current section
                section.end_time = self.current_time;
                self.pending_sections.push(section.clone());
                
                // Identify the next section type based on musical analysis
                let new_type = self.identify_section_type(
                    progress,
                    Some(&section.section_type)
                );
                
                // Start the new section
                self.start_new_section(new_type);
            } else {
                // This is the first section
                let new_type = self.identify_section_type(progress, None);
                self.start_new_section(new_type);
            }
        }
    }

    fn get_energy_change_strength(&self) -> f64 {
        let normalized_energy = self.last_energy / self.max_energy;
        let avg_normalized = self.avg_energy / self.max_energy;
        (normalized_energy - avg_normalized).abs()
    }

    fn get_rhythm_change_strength(&self) -> f64 {
        self.get_rhythm_change_significance()
    }

    fn get_freq_change_strength(&self) -> f64 {
        self.get_freq_profile_change()
    }

    fn analyze_transition_strength(&self, energy_change: f64, rhythm_change: f64, freq_change: f64) -> TransitionStrength {
        // Weight the different changes
        let weighted_change = 
            energy_change * 0.4 +    // Energy changes are important
            rhythm_change * 0.35 +   // Rhythm changes are also significant
            freq_change * 0.25;      // Frequency profile changes support other changes
        
        // Determine transition strength based on weighted change
        if weighted_change > self.config.change_sensitivity * 1.2 { // More sensitive to changes
            TransitionStrength::Strong
        } else if weighted_change > self.config.change_sensitivity {
            TransitionStrength::Medium
        } else {
            TransitionStrength::Weak
        }
    }

    fn supports_section_change(&self, energy: EnergyLevel, freq_profile: FrequencyProfile) -> bool {
        match (energy, freq_profile) {
            // High energy changes often indicate structural changes
            (EnergyLevel::High, _) => true,
            
            // Bass-heavy sections often mark structural points
            (_, FrequencyProfile::BassHeavy) => true,
            
            // Medium energy with balanced profile could be transitional
            (EnergyLevel::Medium, FrequencyProfile::Balanced) => true,
            
            // Other combinations are less likely to be section changes
            _ => false,
        }
    }

    /// Calculate changes in spectral content
    fn calculate_spectral_change(&self) -> f64 {
        if self.freq_distribution_window.len() < 2 {
            return 0.0;
        }

        let current = self.freq_distribution_window.back().unwrap();
        let previous = self.freq_distribution_window
            .iter()
            .rev()
            .nth(1)
            .unwrap();

        // Calculate weighted difference across frequency bands
        let mut weighted_diff = 0.0;
        let weights = [0.8, 0.7, 0.6, 0.5, 0.5, 0.4, 0.3, 0.2]; // More weight to lower frequencies
        
        for (i, ((&curr, &prev), &weight)) in current.iter()
            .zip(previous.iter())
            .zip(weights.iter())
            .enumerate()
        {
            let diff = (curr - prev).abs();
            weighted_diff += diff * weight;
        }

        weighted_diff / weights.iter().sum::<f64>()
    }

    /// Calculate changes in rhythm profile
    fn calculate_rhythm_profile_change(&self) -> f64 {
        if self.rhythm_density_window.len() < 5 {
            return 0.0;
        }

        let recent: Vec<_> = self.rhythm_density_window.iter().rev().take(5).copied().collect();
        let current = recent[0];
        let prev_avg = recent[1..].iter().sum::<f64>() / 4.0;

        // Calculate relative change
        ((current - prev_avg) / prev_avg).abs()
    }

    /// Start a new section
    fn start_new_section(&mut self, section_type: SectionType) {
        // Get base intensity from the current audio features
        let base_intensity = self.calculate_base_intensity();
        
        // Modify intensity based on section type and context
        let adjusted_intensity = match section_type {
            SectionType::Intro => base_intensity.min(0.6),
            SectionType::CutOut => base_intensity.max(0.4),
            SectionType::Verse => base_intensity.min(0.75),
            SectionType::PreChorus => base_intensity.min(0.85),
            SectionType::Chorus => base_intensity.max(0.8),
            SectionType::Bridge => {
                if self.current_time / self.total_duration > 0.7 {
                    base_intensity.max(0.9) // Climactic bridge
                } else {
                    base_intensity.min(0.7) // Regular bridge
                }
            },
            SectionType::Break => base_intensity.min(0.5),
            SectionType::Outro => {
                if base_intensity > 0.8 {
                    base_intensity // High energy outro
                } else {
                    base_intensity * 0.8 // Fade out
                }
            }
        };
        
        self.current_section = Some(PatternSection {
            start_time: self.current_time,
            end_time: 0.0,
            section_type,
            intensity: adjusted_intensity,
        });
    }

    /// Calculate base intensity from current audio features
    fn calculate_base_intensity(&self) -> f64 {
        let energy_factor = if self.max_energy == 0.0 { 
            0.5 
        } else { 
            self.last_energy / self.max_energy 
        };

        let rhythm_factor = self.get_avg_rhythm_density().min(1.0);
        
        // Get frequency band contribution
        let freq_factor = if let Some(latest_freq) = self.freq_distribution_window.back() {
            // Weight different frequency bands
            let bass_weight = 0.4;
            let mid_weight = 0.35;
            let high_weight = 0.25;
            
            let bass = latest_freq[0..2].iter().sum::<f64>() / 2.0;
            let mids = latest_freq[2..5].iter().sum::<f64>() / 3.0;
            let highs = latest_freq[5..8].iter().sum::<f64>() / 3.0;
            
            bass * bass_weight + mids * mid_weight + highs * high_weight
        } else {
            0.5
        };
        
        // Combine factors with weights
        let raw_intensity = 
            energy_factor * 0.4 +
            rhythm_factor * 0.35 +
            freq_factor * 0.25;
            
        // Ensure result is in valid range
        raw_intensity.clamp(0.3, 1.0)
    }

    /// Calculate section intensity based on multiple features
    fn calculate_intensity(&self) -> f64 {
        let energy_factor = if self.max_energy == 0.0 { 0.5 } else { self.last_energy / self.max_energy };
        let rhythm_factor = self.get_avg_rhythm_density().min(1.0);
        
        // Combine factors with weights
        let intensity = (energy_factor * 0.6 + rhythm_factor * 0.4).clamp(0.3, 1.0);
        intensity
    }

    /// Calculate significance of rhythm density change
    fn get_rhythm_change_significance(&self) -> f64 {
        if self.rhythm_density_window.len() < 2 {
            return 0.0;
        }

        let current = *self.rhythm_density_window.back().unwrap();
        let avg_previous = self.rhythm_density_window.iter()
            .rev()
            .skip(1)
            .take(5)
            .sum::<f64>() / 5.0;

        (current - avg_previous).abs() / avg_previous
    }

    /// Determine if a section change should occur
    fn should_change_section(&self, section: &PatternSection) -> bool {
        let section_duration = self.current_time - section.start_time;
        section.duration().as_secs_f64() < section_duration &&
            section_duration >= self.config.min_section_length
    }

    /// Determine the next section type based on song structure analysis
    fn determine_next_section_type(&self, current_type: &SectionType) -> SectionType {
        let progress = self.current_time / self.total_duration;
        
        match current_type {
            SectionType::Intro => SectionType::Verse,
            SectionType::CutOut => SectionType::Verse,
            SectionType::Verse => {
                if progress > 0.7 {
                    SectionType::Bridge
                } else if self.has_high_energy() {
                    SectionType::Chorus
                } else {
                    SectionType::PreChorus
                }
            }
            SectionType::PreChorus => SectionType::Chorus,
            SectionType::Chorus => {
                if progress > 0.8 {
                    SectionType::Outro
                } else {
                    SectionType::Verse
                }
            }
            SectionType::Bridge => {
                if progress > 0.85 {
                    SectionType::Outro
                } else {
                    SectionType::Chorus
                }
            }
            SectionType::Break => {
                if progress > 0.9 {
                    SectionType::Outro
                } else {
                    SectionType::Chorus
                }
            }
            SectionType::Outro => SectionType::Outro, // Stay in outro
        }
    }

    /// Check if current energy levels are high relative to average
    /// Analyze current state to identify the most likely section type
    fn identify_section_type(&self, progress: f64, prev_type: Option<&SectionType>) -> SectionType {
        // Check for sudden drum dropout first
        if self.freq_distribution_window.len() >= 2 {
            let current_drums = self.get_drum_presence(self.freq_distribution_window.back().unwrap());
            let prev_drums = self.get_drum_presence(
                self.freq_distribution_window.iter().rev().nth(1).unwrap()
            );
            
            if prev_drums > 0.6 && current_drums < 0.2 {
                return SectionType::Break;
            }
        }

        let energy_level = self.get_energy_level();
        let rhythm_density = self.get_avg_rhythm_density();
        let (has_strong_bass, has_strong_mids, has_strong_highs) = self.analyze_frequency_bands();
        let freq_balance = self.analyze_frequency_balance();
        
        // Check for sustained drum absence
        let drums_absent = self.calculate_drum_absence();
        let dynamics = self.calculate_dynamics();
        let rhythm_complexity = self.calculate_rhythm_complexity();
        let spectral_contrast = self.calculate_spectral_contrast();

        // If drums have been absent for a while, consider it a break
        if drums_absent > 0.8 && energy_level != EnergyLevel::High {
            return SectionType::Break;
        }
        
        // Early part of the song
        if progress < 0.1 && energy_level != EnergyLevel::High {
            return SectionType::Intro;
        }
        
        // End of the song
        if progress > 0.9 {
            if energy_level == EnergyLevel::High && has_strong_bass && has_strong_mids {
                return SectionType::Chorus; // Final chorus
            }
            return SectionType::Outro;
        }
        
        // Detect break sections first (very distinct characteristics)
        if self.is_likely_break(rhythm_density, dynamics, spectral_contrast) {
            return SectionType::Break;
        }

        // Detect chorus sections (high energy, full spectrum, rich in harmonics)
        if self.is_likely_chorus(energy_level, dynamics, freq_balance, prev_type) {
            return SectionType::Chorus;
        }

        // Now handle other section types
        match (energy_level, rhythm_density, has_strong_bass, has_strong_mids) {
            // High sustained energy with complex patterns - likely bridge or pre-chorus
            (EnergyLevel::High, _, true, true) => {
                if prev_type == Some(&SectionType::Verse) {
                    SectionType::PreChorus
                } else if self.current_time / self.total_duration > 0.6 {
                    SectionType::Bridge
                } else {
                    SectionType::PreChorus
                }
            },
            
            // Medium energy with good rhythm - typical verse
            (EnergyLevel::Medium, _, _, _) => {
                if prev_type == Some(&SectionType::Chorus) || 
                   prev_type == Some(&SectionType::Break) ||
                   prev_type == Some(&SectionType::Bridge) {
                    SectionType::Verse
                } else if rhythm_complexity > 0.7 {
                    SectionType::PreChorus
                } else {
                    SectionType::Verse
                }
            },
            
            // Low energy periods
            (EnergyLevel::Low, low_rhythm, _, _) if low_rhythm < 0.3 => {
                if self.current_time / self.total_duration > 0.7 {
                    SectionType::Bridge
                } else {
                    SectionType::Break
                }
            },
            
            _ => SectionType::Verse,
        }
    }

    fn is_likely_break(&self, rhythm_density: f64, dynamics: f64, spectral_contrast: f64) -> bool {
        // Check for drum pattern drop out
        let has_drums_dropped = self.detect_drum_dropout();

        // Also look at rhythm density for confirmation
        let low_rhythm = rhythm_density < 0.3;

        // Primary detection: Drum pattern dropout
        has_drums_dropped || (low_rhythm && dynamics > 0.6)
    }

    fn detect_drum_dropout(&self) -> bool {
        if self.freq_distribution_window.len() < 5 {
            return false;
        }

        // Get current and previous frequency distributions
        let current = self.freq_distribution_window.back().unwrap();
        let prev_frames: Vec<_> = self.freq_distribution_window.iter()
            .rev()
            .skip(1)
            .take(4)
            .collect();

        // Check kick drum frequency range (50-100 Hz, band 1)
        // and snare drum range (200-400 Hz, bands 2-3)
        let kick_band = 1;
        let snare_bands = [2, 3];

        // Calculate average drum presence in previous frames
        let prev_kick_avg = prev_frames.iter()
            .map(|frame| frame[kick_band])
            .sum::<f64>() / prev_frames.len() as f64;

        let prev_snare_avg = prev_frames.iter()
            .map(|frame| snare_bands.iter().map(|&b| frame[b]).sum::<f64>())
            .sum::<f64>() / prev_frames.len() as f64;

        // Current drum presence
        let current_kick = current[kick_band];
        let current_snare = snare_bands.iter().map(|&b| current[b]).sum::<f64>();

        // Calculate relative drop in drum presence
        let kick_drop = if prev_kick_avg > 0.0 {
            (prev_kick_avg - current_kick) / prev_kick_avg
        } else {
            0.0
        };

        let snare_drop = if prev_snare_avg > 0.0 {
            (prev_snare_avg - current_snare) / prev_snare_avg
        } else {
            0.0
        };

        // Detect significant drum dropout
        // Either both kick and snare drop significantly, or one drops almost completely
        (kick_drop > 0.7 && snare_drop > 0.7) || // Both drums drop out
        kick_drop > 0.9 || // Kick drops completely
        snare_drop > 0.9   // Snare drops completely
    }

    fn is_likely_chorus(&self, energy: EnergyLevel, dynamics: f64, freq_balance: f64, prev_type: Option<&SectionType>) -> bool {
        // Chorus sections typically have:
        // - High sustained energy
        // - Well-balanced frequency spectrum
        // - Often follow verse or pre-chorus
        // - Strong presence across the spectrum
        energy == EnergyLevel::High && 
        dynamics > 0.6 && 
        freq_balance > 0.7 && 
        (prev_type == Some(&SectionType::Verse) || 
         prev_type == Some(&SectionType::PreChorus))
    }

    fn calculate_dynamics(&self) -> f64 {
        if self.energy_window.len() < 2 {
            return 0.0;
        }

        let values: Vec<_> = self.energy_window.iter().copied().collect();
        let max = values.iter().copied().fold(0.0f64, f64::max);
        let min = values.iter().copied().fold(f64::MAX, f64::min);
        
        if max <= 0.0 {
            return 0.0;
        }
        
        (max - min) / max
    }

    fn calculate_rhythm_complexity(&self) -> f64 {
        if self.rhythm_density_window.len() < 5 {
            return 0.0;
        }

        let values: Vec<_> = self.rhythm_density_window.iter().copied().collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        
        // Calculate variance in rhythm
        let variance = values.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / values.len() as f64;

        // Normalize to 0-1 range
        (variance / mean).min(1.0)
    }

    fn get_drum_presence(&self, freq_dist: &[f64]) -> f64 {
        // Check presence of frequencies typical for drums
        let kick_presence = freq_dist[0..2].iter().sum::<f64>() / 2.0;  // 20-150 Hz
        let snare_presence = freq_dist[2..4].iter().sum::<f64>() / 2.0; // 150-500 Hz
        
        // Weight kick drum slightly higher than snare
        kick_presence * 0.6 + snare_presence * 0.4
    }

    fn calculate_drum_absence(&self) -> f64 {
        if self.freq_distribution_window.len() < 5 {
            return 0.0;
        }

        // Look at last 5 frames
        let recent_frames: Vec<_> = self.freq_distribution_window.iter()
            .rev()
            .take(5)
            .collect();

        // Calculate average drum presence over recent frames
        let avg_presence = recent_frames.iter()
            .map(|frame| self.get_drum_presence(frame))
            .sum::<f64>() / 5.0;

        // Return absence score (1.0 means drums completely absent)
        1.0 - avg_presence
    }

    fn calculate_spectral_contrast(&self) -> f64 {
        if self.freq_distribution_window.is_empty() {
            return 0.0;
        }

        let freq_dist = self.freq_distribution_window.back().unwrap();
        let mut peaks = 0;
        let mut valleys = 0;

        for i in 1..freq_dist.len()-1 {
            if freq_dist[i] > freq_dist[i-1] && freq_dist[i] > freq_dist[i+1] {
                peaks += 1;
            }
            if freq_dist[i] < freq_dist[i-1] && freq_dist[i] < freq_dist[i+1] {
                valleys += 1;
            }
        }

        (peaks + valleys) as f64 / freq_dist.len() as f64
    }

    fn analyze_frequency_balance(&self) -> f64 {
        if self.freq_distribution_window.is_empty() {
            return 0.0;
        }

        let freq_dist = self.freq_distribution_window.back().unwrap();
        let total: f64 = freq_dist.iter().sum();
        if total == 0.0 {
            return 0.0;
        }

        // Calculate how evenly energy is distributed
        let ideal_energy = total / freq_dist.len() as f64;
        let variance = freq_dist.iter()
            .map(|&energy| (energy - ideal_energy).powi(2))
            .sum::<f64>() / freq_dist.len() as f64;

        // Convert to balance score (1.0 = perfectly balanced)
        1.0 - (variance / total).min(1.0)
    }
    
    /// Analyze the frequency bands to identify strong components
    fn analyze_frequency_bands(&self) -> (bool, bool, bool) {
        if self.freq_distribution_window.is_empty() {
            return (false, false, false);
        }
        
        // Get the latest frequency distribution
        let latest_freq = self.freq_distribution_window.back().unwrap();
        
        // Check frequency ranges (assuming 8 bands as defined earlier)
        let bass = latest_freq[0..2].iter().sum::<f64>() / 2.0; // Sub-bass and Bass
        let mids = latest_freq[2..5].iter().sum::<f64>() / 3.0; // Low-mids to Upper-mids
        let highs = latest_freq[5..8].iter().sum::<f64>() / 3.0; // Presence to High freq
        
        const STRENGTH_THRESHOLD: f64 = 0.6;
        (
            bass > STRENGTH_THRESHOLD,
            mids > STRENGTH_THRESHOLD,
            highs > STRENGTH_THRESHOLD
        )
    }
    
    /// Get the current energy level category
    fn get_energy_level(&self) -> EnergyLevel {
        let normalized_energy = self.last_energy / self.max_energy;
        if normalized_energy > 0.8 {
            EnergyLevel::High
        } else if normalized_energy > 0.4 {
            EnergyLevel::Medium
        } else {
            EnergyLevel::Low
        }
    }
    
    /// Get the frequency profile type
    fn get_frequency_profile(&self) -> FrequencyProfile {
        if self.freq_distribution_window.is_empty() {
            return FrequencyProfile::Balanced;
        }
        
        let latest_freq = self.freq_distribution_window.back().unwrap();
        
        // Simplified profile analysis
        let bass_energy = latest_freq[0..2].iter().sum::<f64>();
        let mid_energy = latest_freq[2..5].iter().sum::<f64>();
        let high_energy = latest_freq[5..8].iter().sum::<f64>();
        
        if bass_energy > mid_energy && bass_energy > high_energy {
            FrequencyProfile::BassHeavy
        } else if high_energy > bass_energy && high_energy > mid_energy {
            FrequencyProfile::HighHeavy
        } else if mid_energy > bass_energy && mid_energy > high_energy {
            FrequencyProfile::MidHeavy
        } else {
            FrequencyProfile::Balanced
        }
    }

    fn has_high_energy(&self) -> bool {
        self.get_energy_level() == EnergyLevel::High
    }

    /// Get all detected sections, including the current section
    pub fn get_sections(&mut self) -> Vec<PatternSection> {
        let mut sections = self.pending_sections.clone();
        
        // Add the final section if it exists
        if let Some(mut section) = self.current_section.clone() {
            section.end_time = self.current_time;
            sections.push(section);
        }

        // Merge consecutive sections of the same type
        let merged_sections = self.merge_consecutive_sections(sections);
        
        merged_sections
    }

    /// Merge consecutive sections of the same type
    fn merge_consecutive_sections(&self, mut sections: Vec<PatternSection>) -> Vec<PatternSection> {
        if sections.is_empty() {
            return sections;
        }

        let mut merged = Vec::new();
        let mut current_merge = sections.remove(0);
        let mut total_duration = current_merge.end_time - current_merge.start_time;
        let mut section_count = 1;

        for section in sections {
            if section.section_type == current_merge.section_type {
                // Update end time of merged section
                current_merge.end_time = section.end_time;
                // Update intensity based on weighted average
                let new_duration = section.end_time - section.start_time;
                total_duration += new_duration;
                current_merge.intensity = (current_merge.intensity * (total_duration - new_duration) 
                    + section.intensity * new_duration) / total_duration;
                section_count += 1;
            } else {
                // Different section type, push current merge and start new one
                if section_count > 1 {
                    log::info!("Merged {} consecutive {:?} sections", 
                        section_count, current_merge.section_type);
                }
                merged.push(current_merge);
                current_merge = section;
                total_duration = current_merge.end_time - current_merge.start_time;
                section_count = 1;
            }
        }

        // Don't forget to add the last merged section
        if section_count > 1 {
            log::info!("Merged {} consecutive {:?} sections", 
                section_count, current_merge.section_type);
        }
        merged.push(current_merge);

        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_detector() -> SectionDetector {
        SectionDetector::new(SectionConfig::default(), 180.0) // 3-minute test song
    }

    #[test]
    fn test_section_detection() {
        let mut detector = create_test_detector();
        
        // Simulate intro
        for i in 0..50 {
            let time = i as f64 * 0.1;
            detector.process_frame(0.3, vec![0.2; 10], 0.5, time).unwrap();
        }
        
        // Simulate verse (increasing energy)
        for i in 50..100 {
            let time = i as f64 * 0.1;
            detector.process_frame(0.6, vec![0.4; 10], 1.0, time).unwrap();
        }
        
        // Simulate chorus (high energy)
        for i in 100..150 {
            let time = i as f64 * 0.1;
            detector.process_frame(0.9, vec![0.8; 10], 2.0, time).unwrap();
        }
        
        let sections = detector.get_sections();
        assert!(!sections.is_empty());
        
        // Verify section transitions
        assert_eq!(sections[0].section_type, SectionType::Intro);
        assert!(sections.iter().any(|s| s.section_type == SectionType::Verse));
        assert!(sections.iter().any(|s| s.section_type == SectionType::Chorus));
    }

    #[test]
    fn test_intensity_calculation() {
        let mut detector = create_test_detector();
        
        // Low energy
        detector.process_frame(0.2, vec![0.1; 10], 0.5, 0.0).unwrap();
        let sections1 = detector.get_sections();
        assert!(sections1[0].intensity < 0.5);
        
        // High energy
        detector.process_frame(0.9, vec![0.8; 10], 2.0, 1.0).unwrap();
        let sections2 = detector.get_sections();
        assert!(sections2.last().unwrap().intensity > 0.8);
    }

    #[test]
    fn test_minimum_section_length() {
        let mut detector = create_test_detector();
        
        // Process frames with changing energy but shorter than minimum section length
        for i in 0..20 {
            let time = i as f64 * 0.1; // Each frame is 0.1 seconds
            let energy = if i < 10 { 0.3 } else { 0.9 };
            detector.process_frame(energy, vec![0.2; 10], 1.0, time).unwrap();
        }
        
        let sections = detector.get_sections();
        assert_eq!(sections.len(), 1); // Should still be in first section due to minimum length
    }

    #[test]
    fn test_section_merging() {
        // Create consecutive sections manually
        let sections = vec![
            PatternSection {
                start_time: 0.0,
                end_time: 2.0,
                section_type: SectionType::Intro,
                intensity: 0.5,
            },
            PatternSection {
                start_time: 2.0,
                end_time: 4.0,
                section_type: SectionType::Intro,
                intensity: 0.6,
            },
            PatternSection {
                start_time: 4.0,
                end_time: 6.0,
                section_type: SectionType::Verse,
                intensity: 0.7,
            },
            PatternSection {
                start_time: 6.0,
                end_time: 8.0,
                section_type: SectionType::Verse,
                intensity: 0.8,
            },
            PatternSection {
                start_time: 8.0,
                end_time: 10.0,
                section_type: SectionType::Chorus,
                intensity: 0.9,
            },
        ];

        let detector = create_test_detector();
        let merged = detector.merge_consecutive_sections(sections);

        // Should be merged into three sections (merged intros, merged verses, and chorus)
        assert_eq!(merged.len(), 3);
        
        // Check first merged section (intros)
        assert_eq!(merged[0].section_type, SectionType::Intro);
        assert_eq!(merged[0].start_time, 0.0);
        assert_eq!(merged[0].end_time, 4.0);
        // Intensity should be weighted average
        assert!((merged[0].intensity - 0.55).abs() < 0.01);

        // Check second merged section (verses)
        assert_eq!(merged[1].section_type, SectionType::Verse);
        assert_eq!(merged[1].start_time, 4.0);
        assert_eq!(merged[1].end_time, 8.0);
        assert!((merged[1].intensity - 0.75).abs() < 0.01);

        // Check unchanged chorus section
        assert_eq!(merged[2].section_type, SectionType::Chorus);
        assert_eq!(merged[2].start_time, 8.0);
        assert_eq!(merged[2].end_time, 10.0);
        assert_eq!(merged[2].intensity, 0.9);
    }
}
