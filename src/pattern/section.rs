use crate::audio::analysis::AudioFeatures;
use crate::common::Result;
use crate::pattern::section_detection::{
    AudioSectionDetector, SectionBoundary, SectionDetectionConfig,
};
use crate::pattern::types::{PatternSection, SectionType};

/// Configuration for section generation
#[derive(Debug, Clone)]
pub struct SectionConfig {
    /// Minimum section duration in seconds
    pub min_section_duration: f64,
    /// Base intensity for section types
    pub base_intensities: Vec<(SectionType, f64)>,
    /// Minimum time between intensity changes
    pub min_intensity_interval: f64,
    /// Configuration for audio-based section detection
    pub detection_config: SectionDetectionConfig,
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
            detection_config: SectionDetectionConfig::default(),
        }
    }
}

/// Manages section generation and transitions
#[derive(Debug)]
pub struct SectionManager {
    config: SectionConfig,
    total_duration: f64,
    section_detector: AudioSectionDetector,
    detected_sections: Vec<SectionBoundary>,
    final_sections: Vec<PatternSection>,
}

impl SectionManager {
    /// Creates a new SectionManager
    pub fn new(config: SectionConfig, total_duration: f64) -> Self {
        Self {
            section_detector: AudioSectionDetector::new(config.detection_config.clone()),
            total_duration,
            config,
            detected_sections: Vec::new(),
            final_sections: Vec::new(),
        }
    }

    /// Process new audio features
    pub fn process_features(&mut self, features: &AudioFeatures, time: f64) -> Result<()> {
        if let Some(boundary) = self.section_detector.process_features(features, time)? {
            self.detected_sections.push(boundary);
            self.update_sections()?;
        }
        Ok(())
    }

    /// Update sections based on new detection
    fn update_sections(&mut self) -> Result<()> {
        if self.detected_sections.len() >= 2 {
            let sections = self.generate_sections_from_detected()?;
            self.final_sections = sections;
        }
        Ok(())
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

    /// Generate sections for the entire song
    pub fn generate_sections(&mut self) -> Result<Vec<PatternSection>> {
        // If we have detected sections, use them
        if !self.detected_sections.is_empty() {
            self.generate_sections_from_detected()
        } else {
            // Fall back to rule-based generation if no sections detected
            self.generate_default_sections()
        }
    }

    /// Generate sections from detected boundaries
    fn generate_sections_from_detected(&self) -> Result<Vec<PatternSection>> {
        let mut sections = Vec::new();
        let mut last_time = 0.0;

        // Process each detected section boundary
        for (i, boundary) in self.detected_sections.iter().enumerate() {
            // Skip very short sections
            if boundary.time - last_time < self.config.min_section_duration {
                continue;
            }

            // Determine section type based on audio features and sequence
            let section_type = if i == 0 {
                SectionType::Intro
            } else {
                self.refine_section_type(&boundary.boundary_type, &sections)
            };

            sections.push(PatternSection::new(
                last_time,
                boundary.time,
                section_type,
                self.calculate_audio_based_intensity(boundary),
            ));

            last_time = boundary.time;
        }

        // Add final section if needed
        if last_time < self.total_duration {
            sections.push(PatternSection::new(
                last_time,
                self.total_duration,
                SectionType::Outro,
                self.get_base_intensity(&SectionType::Outro),
            ));
        }

        Ok(sections)
    }

    /// Generate default sections when no audio detection is available
    fn generate_default_sections(&self) -> Result<Vec<PatternSection>> {
        let mut sections = Vec::new();

        // Add intro section
        let intro_duration = self.config.min_section_duration * 1.5;
        sections.push(PatternSection::new(
            0.0,
            intro_duration,
            SectionType::Intro,
            self.get_base_intensity(&SectionType::Intro),
        ));

        let mut current_section_start = intro_duration;

        // Main song sections
        while current_section_start < self.total_duration - self.config.min_section_duration * 2.0 {
            let section_type = self.determine_next_section(&sections);
            let duration = self.calculate_section_duration(&section_type);

            sections.push(PatternSection::new(
                current_section_start,
                current_section_start + duration,
                section_type,
                self.get_base_intensity(&section_type),
            ));

            current_section_start += duration;
        }

        // Add outro section
        sections.push(PatternSection::new(
            current_section_start,
            self.total_duration,
            SectionType::Outro,
            self.get_base_intensity(&SectionType::Outro),
        ));

        Ok(sections)
    }

    /// Determine the next section type based on sequence and audio features
    fn determine_next_section(&self, current_sections: &[PatternSection]) -> SectionType {
        if let Some(last_section) = current_sections.last() {
            match last_section.section_type {
                SectionType::Intro => SectionType::Verse,
                SectionType::Verse | SectionType::Verse2 => {
                    if current_sections.len() < 3 {
                        SectionType::PreChorus
                    } else {
                        SectionType::Bridge
                    }
                }
                SectionType::PreChorus => SectionType::Chorus,
                SectionType::Chorus | SectionType::PostChorus | SectionType::Drop => {
                    SectionType::Verse
                }
                SectionType::Bridge | SectionType::Breakdown => SectionType::Chorus,
                SectionType::BuildUp => SectionType::Drop,
                SectionType::Outro => SectionType::Verse,
            }
        } else {
            SectionType::Verse
        }
    }

    /// Refine section type based on audio features and musical structure
    fn refine_section_type(
        &self,
        detected_type: &SectionType,
        current_sections: &[PatternSection],
    ) -> SectionType {
        if let Some(last_section) = current_sections.last() {
            match (last_section.section_type, detected_type) {
                // Don't allow consecutive choruses unless strong detection
                (SectionType::Chorus, SectionType::Chorus) => SectionType::Verse,
                // Enforce pre-chorus before chorus if pattern exists
                (SectionType::Verse, SectionType::Chorus) => {
                    if !current_sections
                        .iter()
                        .any(|s| matches!(s.section_type, SectionType::PreChorus))
                    {
                        SectionType::PreChorus
                    } else {
                        *detected_type
                    }
                }
                // Keep detected type in other cases
                _ => *detected_type,
            }
        } else {
            *detected_type
        }
    }

    /// Calculate appropriate duration for a section
    fn calculate_section_duration(&self, section_type: &SectionType) -> f64 {
        let base_duration = match *section_type {
            SectionType::Intro | SectionType::Outro => self.config.min_section_duration * 1.5,
            SectionType::PreChorus
            | SectionType::Bridge
            | SectionType::Breakdown
            | SectionType::BuildUp => self.config.min_section_duration,
            SectionType::Verse
            | SectionType::Verse2
            | SectionType::Chorus
            | SectionType::PostChorus
            | SectionType::Drop => self.config.min_section_duration * 2.0,
        };

        // Add some variation (±20%)
        let variation = (fastrand::f64() - 0.5) * 0.4;
        (base_duration * (1.0 + variation)).max(self.config.min_section_duration)
    }

    /// Calculate section intensity based on audio features
    fn calculate_audio_based_intensity(&self, boundary: &SectionBoundary) -> f64 {
        let base_intensity = self.get_base_intensity(&boundary.boundary_type);

        // Adjust intensity based on boundary confidence and novelty
        let confidence_factor = boundary.confidence;
        let novelty_factor = (boundary.novelty_score / 2.0).min(1.0);

        // Combine factors with base intensity
        let combined_intensity =
            base_intensity * 0.6 + confidence_factor * 0.2 + novelty_factor * 0.2;

        combined_intensity.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_features(bass: f32, mid: f32, high: f32) -> AudioFeatures {
        AudioFeatures {
            rms: 0.5,
            centroid: 1000.0,
            spread: 500.0,
            bass_energy: bass,
            mid_energy: mid,
            high_energy: high,
            rolloff: 2000.0,
            zero_crossing_rate: 0.1,
        }
    }

    #[test]
    fn test_section_generation() {
        let config = SectionConfig::default();
        let total_duration = 60.0; // 1 minute song
        let mut manager = SectionManager::new(config.clone(), total_duration);

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
    fn test_audio_based_detection() {
        let config = SectionConfig::default();
        let total_duration = 60.0;
        let mut manager = SectionManager::new(config, total_duration);

        // Simulate audio features for different sections
        let test_cases = vec![
            (0.0, create_test_features(0.3, 0.3, 0.3)),  // Intro-like
            (10.0, create_test_features(0.8, 0.7, 0.6)), // High energy - Chorus-like
            (20.0, create_test_features(0.4, 0.4, 0.3)), // Medium energy - Verse-like
            (30.0, create_test_features(0.7, 0.8, 0.8)), // High energy, high freq - Bridge-like
            (40.0, create_test_features(0.3, 0.2, 0.2)), // Low energy - Outro-like
        ];

        // Process features
        for (time, features) in test_cases {
            manager.process_features(&features, time).unwrap();
        }

        let sections = manager.generate_sections().unwrap();

        // Verify sections were created
        assert!(!sections.is_empty());

        // Check for expected section sequence
        if sections.len() >= 4 {
            // First section should be intro
            assert_eq!(sections[0].section_type, SectionType::Intro);

            // High energy section should be detected
            let has_high_energy = sections
                .iter()
                .any(|s| matches!(s.section_type, SectionType::Chorus));
            assert!(has_high_energy, "No chorus section detected");

            // Should include at least one verse
            let has_verse = sections
                .iter()
                .any(|s| matches!(s.section_type, SectionType::Verse));
            assert!(has_verse, "No verse section detected");

            // Last section should be outro
            assert_eq!(sections.last().unwrap().section_type, SectionType::Outro);
        }
    }

    #[test]
    fn test_section_transitions() {
        let config = SectionConfig::default();
        let total_duration = 120.0; // 2 minute song
        let mut manager = SectionManager::new(config, total_duration);

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

    #[test]
    fn test_audio_based_intensity() {
        let config = SectionConfig::default();
        let total_duration = 60.0;
        let manager = SectionManager::new(config, total_duration);

        // Test high energy section
        let high_energy_boundary = SectionBoundary {
            time: 10.0,
            novelty_score: 0.8,
            boundary_type: SectionType::Chorus,
            confidence: 0.9,
        };

        let intensity = manager.calculate_audio_based_intensity(&high_energy_boundary);
        assert!(
            intensity > 0.8,
            "High energy section should have high intensity"
        );

        // Test low energy section
        let low_energy_boundary = SectionBoundary {
            time: 20.0,
            novelty_score: 0.3,
            boundary_type: SectionType::Verse,
            confidence: 0.6,
        };

        let intensity = manager.calculate_audio_based_intensity(&low_energy_boundary);
        assert!(
            intensity < 0.8,
            "Low energy section should have lower intensity"
        );
    }
}
