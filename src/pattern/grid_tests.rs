#[cfg(test)]
mod grid_alignment_tests {
    use super::*;
    use crate::audio::analysis::tempo::TimeSignature;
    use crate::audio::AnalysisResults;
    use assert_approx_eq::assert_approx_eq;

    fn create_test_analysis(bpm: f64, duration: f64) -> AnalysisResults {
        AnalysisResults {
            bpm,
            tempo_confidence: 1.0,
            time_signature: TimeSignature::default(),
            onset_times: vec![0.0, duration/2.0, duration],
            onset_strengths: vec![1.0, 1.0, 1.0],
            onset_features: vec![],
            avg_features: None,
        }
    }

    #[test]
    fn test_exact_grid_alignment() {
        let bpm = 174.0;
        let beat_duration = 60.0 / bpm;
        let config = GeneratorConfig::default();
        let mut generator = PatternGenerator::new(config, bpm, "test".to_string()).unwrap();
        
        // Test for 4 beats
        let analysis = create_test_analysis(bpm, beat_duration * 4.0);
        let pattern = generator.generate_pattern(&analysis).unwrap();
        
        // Check exact beat positions
        for (i, note) in pattern.notes.iter().enumerate() {
            let expected_time = (i as f64 * beat_duration * 1_000_000.0).round() / 1_000_000.0;
            assert_approx_eq!(note.timestamp, expected_time, 0.000001);
        }
    }

    #[test]
    fn test_subdivisions_alignment() {
        let bpm = 174.0;
        let config = GeneratorConfig::default();
        let mut generator = PatternGenerator::new(config, bpm, "test".to_string()).unwrap();
        
        // Test sixteenth notes
        generator.set_grid_division(GridDivision::Sixteenth);
        let analysis = create_test_analysis(bpm, 60.0 / bpm); // One beat duration
        let pattern = generator.generate_pattern(&analysis).unwrap();
        
        // Should have 4 notes per beat for sixteenth notes
        assert_eq!(pattern.notes.len(), 4);
        
        // Check timing
        let sixteenth_duration = 60.0 / bpm / 4.0;
        for (i, note) in pattern.notes.iter().enumerate() {
            let expected_time = (i as f64 * sixteenth_duration * 1_000_000.0).round() / 1_000_000.0;
            assert_approx_eq!(note.timestamp, expected_time, 0.000001);
        }
    }

    #[test]
    fn test_grid_consistency() {
        let bpm = 174.0;
        let config = GeneratorConfig::default();
        let mut generator = PatternGenerator::new(config, bpm, "test".to_string()).unwrap();
        
        // Test for 8 beats
        let analysis = create_test_analysis(bpm, 60.0 / bpm * 8.0);
        let pattern = generator.generate_pattern(&analysis).unwrap();
        
        // Check consistent spacing between notes
        let expected_spacing = 60.0 / bpm;
        for notes in pattern.notes.windows(2) {
            let spacing = notes[1].timestamp - notes[0].timestamp;
            assert_approx_eq!(spacing, expected_spacing, 0.000001);
        }
    }
}