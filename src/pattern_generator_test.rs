#[cfg(test)]
mod tests {
    use super::*;

    fn create_basic_analysis() -> AnalysisResults {
        AnalysisResults {
            bpm: 120.0,
            confidence: 0.9,
            notes: vec![],
            onset_times: vec![0.0, 0.5, 1.0, 1.5, 2.0],
            beat_times: vec![0.0, 0.5, 1.0, 1.5, 2.0],
            beat_strengths: vec![1.0, 0.5, 1.0, 0.5, 1.0],
        }
    }

    #[test]
    fn test_pattern_generation() {
        let config = GeneratorConfig::default();
        let mut generator = PatternGenerator::new(config, 120.0).unwrap();
        let analysis = create_basic_analysis();

        let pattern = generator.generate_pattern(&analysis).unwrap();
        assert!(!pattern.notes.is_empty());
        assert!(!pattern.sections.is_empty());
        assert_eq!(pattern.metadata.bpm, 120.0);
    }

    #[test]
    fn test_note_spacing() {
        let config = GeneratorConfig::default();
        let mut generator = PatternGenerator::new(config, 120.0).unwrap();
        let analysis = create_basic_analysis();

        let pattern = generator.generate_pattern(&analysis).unwrap();
        
        for i in 1..pattern.notes.len() {
            let time_diff = pattern.notes[i].timestamp - pattern.notes[i-1].timestamp;
            assert!(time_diff >= 0.0);
        }
    }

    #[test]
    fn test_grid_snapping() {
        let config = GeneratorConfig::default();
        let mut generator = PatternGenerator::new(config, 120.0).unwrap();
        let analysis = AnalysisResults {
            bpm: 120.0,
            confidence: 0.9,
            notes: vec![],
            onset_times: vec![
                0.1,  // Should snap to 0.0
                0.35, // Should snap to 0.25
                0.55, // Should snap to 0.5
                0.8,  // Should snap to 0.75
                1.1   // Should snap to 1.0
            ],
            beat_times: vec![0.0, 0.5, 1.0],
            beat_strengths: vec![1.0, 0.5, 1.0],
        };

        let pattern = generator.generate_pattern(&analysis).unwrap();
        let beat_interval = 60.0 / 120.0;

        let expected_times = vec![0.0, 0.25, 0.5, 0.75, 1.0].iter()
            .map(|&x| x * beat_interval)
            .collect::<Vec<_>>();

        assert_eq!(pattern.notes.len(), expected_times.len());
        
        for &expected_time in &expected_times {
            assert!(
                pattern.notes.iter().any(|n| (n.timestamp - expected_time).abs() < 0.01),
                "Missing expected note at time {}",
                expected_time
            );
        }
    }

    #[test]
    fn test_snapping_thresholds() {
        let config = GeneratorConfig::default();
        let mut generator = PatternGenerator::new(config, 120.0).unwrap();
        let analysis = AnalysisResults {
            bpm: 120.0,
            confidence: 0.9,
            notes: vec![],
            onset_times: vec![
                0.05,  // Should snap to 0.0 (within 0.125)
                0.15,  // Should snap to 0.25 (within boundary)
                0.4,   // Should snap to 0.5
                0.85,  // Should snap to 0.75
                0.95   // Should snap to 1.0
            ],
            beat_times: vec![0.0, 0.5, 1.0],
            beat_strengths: vec![1.0, 0.5, 1.0],
        };

        let pattern = generator.generate_pattern(&analysis).unwrap();
        let beat_interval = 60.0 / 120.0;

        for note in &pattern.notes {
            let normalized_pos = note.timestamp / beat_interval;
            let decimal = normalized_pos.fract();
            
            assert!(
                decimal.abs() < 0.01 || // On beat
                (decimal - 0.25).abs() < 0.01 || // Quarter
                (decimal - 0.5).abs() < 0.01 || // Half
                (decimal - 0.75).abs() < 0.01,  // Three quarters
                "Note at {} not properly snapped", note.timestamp
            );
        }
    }
}
