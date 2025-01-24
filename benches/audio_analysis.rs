use crate::{
    AnalysisConfig, AudioAnalyzer, {FeatureConfig, FeatureExtractor, OnsetConfig, OnsetDetector},
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::f32::consts::PI;

fn generate_test_signal(sample_rate: u32, duration: f32) -> Vec<f32> {
    let num_samples = (sample_rate as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        // Mix of frequencies with amplitude modulation
        let base_freq = if (t * 4.0).floor() % 2.0 == 0.0 {
            440.0 // A4
        } else {
            880.0 // A5
        };

        let am = (2.0 * PI * 4.0 * t).sin() * 0.5 + 0.5; // 4 Hz amplitude modulation
        let sample = am
            * (0.5 * (2.0 * PI * base_freq * t).sin()
                + 0.3 * (2.0 * PI * base_freq * 2.0 * t).sin()
                + 0.2 * (2.0 * PI * base_freq * 3.0 * t).sin());

        samples.push(sample);
    }

    samples
}

fn bench_onset_detection(c: &mut Criterion) {
    let config = OnsetConfig::default();
    let mut detector = OnsetDetector::new(config).unwrap();
    let samples = generate_test_signal(44100, 0.1); // 100ms of audio

    c.bench_function("onset_detection", |b| {
        b.iter(|| {
            detector.process(black_box(&samples)).unwrap();
        });
    });
}

fn bench_feature_extraction(c: &mut Criterion) {
    let config = FeatureConfig::default();
    let mut extractor = FeatureExtractor::new(config, 44100).unwrap();
    let samples = generate_test_signal(44100, 0.1);

    c.bench_function("feature_extraction", |b| {
        b.iter(|| {
            extractor.process(black_box(&samples)).unwrap();
        });
    });
}

fn bench_full_analysis(c: &mut Criterion) {
    let config = AnalysisConfig::default();
    let mut analyzer = AudioAnalyzer::new(config, 44100).unwrap();
    let samples = generate_test_signal(44100, 0.1);

    c.bench_function("full_analysis", |b| {
        b.iter(|| {
            analyzer.process_chunk(black_box(&samples)).unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_onset_detection,
    bench_feature_extraction,
    bench_full_analysis
);
criterion_main!(benches);
