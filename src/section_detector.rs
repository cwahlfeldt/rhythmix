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

const SECTION_ANALYSIS_WINDOW: usize = 4;
const ENERGY_THRESHOLD_MULT: f64 = 1.2;
const MIN_SECTION_LENGTH: f64 = 4.0;

pub struct SectionConfig {
    pub window_size: f64,
    pub min_section_length: f64,
    pub energy_threshold_mult: f64,
    pub section_length_mult: f64,
    pub change_sensitivity: f64,
}

impl Default for SectionConfig {
    fn default() -> Self {
        Self {
            window_size: SECTION_ANALYSIS_WINDOW as f64,
            min_section_length: MIN_SECTION_LENGTH,
            energy_threshold_mult: ENERGY_THRESHOLD_MULT,
            section_length_mult: 1.5,
            change_sensitivity: 0.2,
        }
    }
}

pub struct SectionDetector {
    config: SectionConfig,
    energy_window: VecDeque<f64>,
    freq_window: VecDeque<Vec<f64>>,
    current_time: f64,
    current_section: Option<PatternSection>,
    sections: Vec<PatternSection>,
    max_energy: f64,
    total_duration: f64,
}

impl SectionDetector {
    pub fn new(config: SectionConfig, total_duration: f64) -> Self {
        Self {
            config,
            energy_window: VecDeque::new(),
            freq_window: VecDeque::new(),
            current_time: 0.0,
            current_section: None,
            sections: Vec::new(),
            max_energy: 0.0,
            total_duration,
        }
    }

    pub fn process_frame(&mut self, energy: f64, frequencies: Vec<f64>, time: f64) {
        self.current_time = time;
        self.max_energy = self.max_energy.max(energy);
        
        // Update windows
        let window_size = (self.config.window_size * 10.0) as usize;
        self.energy_window.push_back(energy);
        self.freq_window.push_back(frequencies);
        
        while self.energy_window.len() > window_size {
            self.energy_window.pop_front();
        }
        while self.freq_window.len() > window_size {
            self.freq_window.pop_front();
        }
        
        self.detect_section_change();
    }

    fn get_energy_level(&self) -> EnergyLevel {
        let normalized = self.energy_window.back().unwrap_or(&0.0) / self.max_energy;
        if normalized > 0.8 {
            EnergyLevel::High
        } else if normalized > 0.4 {
            EnergyLevel::Medium
        } else {
            EnergyLevel::Low
        }
    }

    fn get_frequency_profile(&self) -> FrequencyProfile {
        if let Some(freq) = self.freq_window.back() {
            let bass = freq[0..2].iter().sum::<f64>();
            let mid = freq[2..5].iter().sum::<f64>();
            let high = freq[5..].iter().sum::<f64>();
            
            if bass > mid && bass > high {
                FrequencyProfile::BassHeavy
            } else if mid > bass && mid > high {
                FrequencyProfile::MidHeavy
            } else if high > bass && high > mid {
                FrequencyProfile::HighHeavy
            } else {
                FrequencyProfile::Balanced
            }
        } else {
            FrequencyProfile::Balanced
        }
    }

    fn identify_section_type(&self) -> SectionType {
        let progress = self.current_time / self.total_duration;
        let energy = self.get_energy_level();
        let freq_profile = self.get_frequency_profile();

        match (energy, freq_profile, progress) {
            // Start of song
            (_, _, p) if p < 0.1 => SectionType::Intro,
            
            // End of song
            (_, _, p) if p > 0.9 => SectionType::Outro,
            
            // High energy sections
            (EnergyLevel::High, FrequencyProfile::BassHeavy, _) => SectionType::Chorus,
            (EnergyLevel::High, _, p) if p > 0.6 => SectionType::Bridge,
            (EnergyLevel::High, _, _) => SectionType::Chorus,
            
            // Medium energy sections
            (EnergyLevel::Medium, FrequencyProfile::BassHeavy, _) => SectionType::Verse,
            (EnergyLevel::Medium, _, p) if p > 0.4 && p < 0.6 => SectionType::PreChorus,
            (EnergyLevel::Medium, _, _) => SectionType::Verse,
            
            // Low energy sections
            (EnergyLevel::Low, _, p) if p > 0.7 => SectionType::Bridge,
            _ => SectionType::Verse,
        }
    }

    fn detect_section_change(&mut self) {
        let should_change = match &self.current_section {
            None => true,
            Some(section) => {
                let duration = self.current_time - section.start_time;
                duration >= self.config.min_section_length
            }
        };

        if should_change {
            if let Some(mut section) = self.current_section.take() {
                section.end_time = self.current_time;
                self.sections.push(section);
            }
            
            let new_type = self.identify_section_type();
            let base_intensity = self.energy_window.back().unwrap_or(&0.0) / self.max_energy;
            
            self.current_section = Some(PatternSection {
                start_time: self.current_time,
                end_time: 0.0,
                section_type: new_type,
                intensity: base_intensity.max(0.3),
            });
        }
    }

    pub fn get_sections(&mut self) -> Vec<PatternSection> {
        let mut sections = self.sections.clone();
        
        if let Some(mut section) = self.current_section.take() {
            section.end_time = self.current_time;
            sections.push(section);
        }
        
        sections
    }
}
