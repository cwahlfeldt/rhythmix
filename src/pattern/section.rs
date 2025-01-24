use crate::common::Result;
use crate::pattern::types::{PatternSection, SectionType};
use std::cmp::Ordering;

/// Configuration for section generation
#[derive(Debug, Clone)]
pub struct SectionConfig {
    /// Minimum section duration in seconds
    pub min_section_duration: f64,
    /// Base intensity for section types
    pub base_intensities: Vec<(SectionType, f64)>,
    /// Minimum time between intensity changes
    pub min_intensity_interval: f64,
}

impl Default for SectionConfig {
    fn default() -> Self {
        Self {
            min_section_duration: 8.0,
            base_intensities: vec![
                (SectionType::Intro, 0.4),
                (SectionType::Verse, 0.6),
                (SectionType::PreChorus, 0.7),
                (SectionType::Chorus, 0.9),
                (SectionType::Bridge, 0.8),
                (SectionType::Outro, 0.5),
            ],
            min_intensity_interval: 2.0,
        }
    }
}

/// Manages section generation and transitions
#[derive(Debug)]
pub struct SectionManager {
    config: SectionConfig,
    total_duration: f64,
}

impl SectionManager {
    /// Creates a new SectionManager
    pub fn new(config: SectionConfig, total_duration: f64) -> Self {
        Self {
            config,
            total_duration,
        }
    }

    /// Generate initial sections for the entire song
    pub fn generate_sections(&self) -> Result<Vec<PatternSection>> {
        let mut sections = Vec::new();
        let mut current_time = 0.0;

        // Add intro section
        let intro_duration = self.config.min_section_duration * 1.5;
        sections.push(PatternSection::new(
            0.0,
            intro_duration,
            SectionType::Intro,
            self.get_base_intensity(&SectionType::Intro),
        ));
        current_time = intro_duration;

        // Main song sections
        while current_time < self.total_duration - self.config.min_section_duration * 2.0 {
            let section_type = self.determine_next_section(&sections);
            let duration = self.calculate_section_duration(&section_type);
            
            sections.push(PatternSection::new(
                current_time,
                current_time + duration,
                section_type,
                self.get_base_intensity(&section_type),
            ));
            
            current_time += duration;
        }

        // Add outro section
        sections.push(PatternSection::new(
            current_time,
            self.total_duration,
            SectionType::Outro,
            self.get_base_intensity(&SectionType::Outro),
        ));

        Ok(sections)
    }

    /// Get base intensity for a section type
    fn get_base_intensity(&self, section_type: &SectionType) -> f64 {
        self.config
            .base_intensities
            .iter()
            .find(|(st, _)| st == section_type)
            .map(|(_, intensity)| *intensity)
            .unwrap_or(0.5)
    }

    /// Determine the next section type based on current sequence
    fn determine_next_section(&self, current_sections: &[PatternSection]) -> SectionType {
        if let Some(last_section) = current_sections.last() {
            match last_section.section_type {
                SectionType::Intro => SectionType::Verse,
                SectionType::Verse => {
                    if current_sections.len() < 3 {
                        SectionType::PreChorus
                    } else {
                        SectionType::Bridge
                    }
                }
                SectionType::PreChorus => SectionType::Chorus,
                SectionType::Chorus => SectionType::Verse,
                SectionType::Bridge => SectionType::Chorus,
                SectionType::Outro => SectionType::Verse,
            }
        } else {
            SectionType::Verse
        }
    }

    /// Calculate appropriate duration for a section
    fn calculate_section_duration(&self, section_type: &SectionType) -> f64 {
        let base_duration = match section_type {
            SectionType::Intro | SectionType::Outro => self.config.min_section_duration * 1.5,
            SectionType::PreChorus | SectionType::Bridge => self.config.min_section_duration,
            SectionType::Verse | SectionType::Chorus => self.config.min_section_duration * 2.0,
        };

        // Add some variation (±20%)
        let variation = (fastrand::f64() - 0.5) * 0.4;
        (base_duration * (1.0 + variation)).max(self.config.min_section_duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_generation() {
        let config = SectionConfig::default();
        let total_duration = 60.0; // 1 minute song
        let manager = SectionManager::new(config.clone(), total_duration);

        let sections = manager.generate_sections().unwrap();

        // Verify basic properties
        assert!(!sections.is_empty());
        assert_eq!(sections[0].section_type, SectionType::Intro);
        assert_eq!(sections.last().unwrap().section_type, SectionType::Outro);
        
        // Check section timing
        for window in sections.windows(2) {
            assert_eq!(window[0].end_time, window[1].start_time);
            assert!(window[0].duration() >= config.min_section_duration);
        }

        // Verify full coverage
        assert_eq!(sections[0].start_time, 0.0);
        assert!((sections.last().unwrap().end_time - total_duration).abs() < 0.001);
    }

    #[test]
    fn test_section_transitions() {
        let config = SectionConfig::default();
        let total_duration = 120.0; // 2 minute song
        let manager = SectionManager::new(config, total_duration);

        let sections = manager.generate_sections().unwrap();

        // Check for logical section transitions
        for window in sections.windows(2) {
            match (&window[0].section_type, &window[1].section_type) {
                (SectionType::PreChorus, section_type) => {
                    assert_eq!(section_type, &SectionType::Chorus);
                }
                (SectionType::Chorus, section_type) => {
                    assert!(matches!(
                        section_type,
                        SectionType::Verse | SectionType::Bridge | SectionType::Outro
                    ));
                }
                _ => {}
            }
        }
    }
}
