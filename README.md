# Rhythmix

A high-performance audio analysis server that transforms audio files into rhythm game pattern data. Built with Rust for maximum performance and accuracy.

## Overview

Rhythmix is an HTTP server that processes MP3 and WAV files to generate synchronized rhythm game patterns. It performs beat detection, tempo analysis, and intelligent pattern generation to create engaging gameplay experiences.

### Key Features

- Real-time audio analysis with precise beat detection
- Automatic BPM detection and difficulty scaling
- Dynamic pattern generation based on audio characteristics
- Simple RESTful API for easy integration
- Support for MP3 and WAV formats
- Comprehensive error handling and validation
- Configurable pattern complexity

## Architecture

The project follows a flat structure with focused modules for complex audio processing and pattern generation:

```
src/
├── main.rs              # Application entry point and configuration
├── server.rs            # HTTP server implementation
├── handlers.rs          # Request handlers and response formatting
├── audio_decoder.rs     # Audio file decoding (MP3/WAV)
├── audio_analyzer.rs    # Beat detection and BPM analysis
├── fft_processor.rs     # FFT and frequency analysis
├── onset_detector.rs    # Note onset detection and timing
├── pattern_generator.rs # Core pattern generation logic
├── pattern_types.rs     # Pattern definitions and behaviors
├── difficulty.rs        # Difficulty scaling and calculations
├── thread_pool.rs       # Custom thread pool for audio processing
├── error.rs            # Error types and handling
└── types.rs            # Shared types and constants
```

Key responsibilities:
- `server.rs` & `handlers.rs`: Handle HTTP interactions and file uploads
- Audio Processing:
  - `audio_decoder.rs`: Efficient audio file decoding
  - `audio_analyzer.rs`: Core analysis and beat detection
  - `fft_processor.rs`: Frequency domain analysis
  - `onset_detector.rs`: Precise note timing detection
- Pattern Generation:
  - `pattern_generator.rs`: Creates gameplay patterns
  - `pattern_types.rs`: Defines note types and combinations
  - `difficulty.rs`: Scales patterns based on BPM/complexity
- Infrastructure:
  - `thread_pool.rs`: Manages concurrent audio processing
  - `error.rs`: Custom error handling
  - `types.rs`: Shared data structures

## Implementation Details

### Audio Analysis Module

- Uses `rodio` for audio decoding
- Implements Fast Fourier Transform (FFT) for frequency analysis
- Onset detection for precise beat timing
- Multi-threaded processing for optimal performance

### Pattern Generation

- Dynamic difficulty scaling based on BPM
- Multiple pattern types:
  - Single notes - Currently
  - Hold notes - Planned
  - Simultaneous notes - Planned
  - Sliding patterns - Planned
- Intelligent pattern spacing based on human reaction time
- Difficulty curves that respect musical phrases

### HTTP Server

- Built with `hyper` for high-performance async I/O
- JSON response format represents game notes that sync with music:

```json
{
  "metadata": {
    "bpm": 128.5,
    "duration": 180.0,
    "difficulty": 0.75,
    "recommended_scroll_speed": 2.1
  },
  "notes": [
    {
      "timestamp": 1.25,        // When the note should be clicked (seconds)
      "type": "tap",           // Single tap note
      "lane": 2               // Position (0-3 for a 3-lane game)
    },
    {
      "timestamp": 3.0,        // When the note should be clicked (seconds)
      "type": "tap",           // Single tap note
      "lane": 0               // Position (0-3 for a 3-lane game)
    },
    {
      "timestamp": 5.25,        // When the note should be clicked (seconds)
      "type": "tap",           // Single tap note
      "lane": 1               // Position (0-3 for a 3-lane game)
    },
    {
      "timestamp": 10,        // When the note should be clicked (seconds)
      "type": "tap",           // Single tap note
      "lane": 2               // Position (0-3 for a 3-lane game)
    },
    ...
  ],
  "sections": [
    {
      "start_time": 0.0,
      "end_time": 30.0,
      "section_type": "verse",
      "intensity": 0.6        // Affects pattern density
    },
    ...
  ],
  "beat_markers": [          // For visual effects/timing
    {
      "timestamp": 0.0,
      "is_strong_beat": true  // First beat of measure
    },
    ...
  ]
}
```

Note types and gameplay elements:
- `tap`: Basic single-click note
- `hold`: Must be pressed and held for duration
- `slide`: Requires directional swipe
- `multi`: Multiple simultaneous notes
- `lane`: Position (0-3 typical for 4-lane layout)
- `timestamp`: Precise timing for note hit (in seconds)
- `sections`: Song structure affecting pattern intensity
- `beat_markers`: For visual effects and player feedback

The response is designed to:
1. Provide precise timing for note placement
2. Support various gameplay mechanics
3. Sync perfectly with audio beats
4. Scale difficulty with BPM
5. Enable engaging pattern combinations

## Setup and Installation

1. Ensure you have Rust installed (1.75.0 or newer)
2. Clone the repository:
```bash
git clone https://github.com/yourusername/rhythmix.git
cd rhythmix
```

3. Install dependencies:
```bash
cargo build --release
```

4. Run the server:
```bash
cargo run --release
```

The server will start on `localhost:3000` by default.

## API Usage

### Upload Audio File

```bash
curl -X POST http://localhost:3000/analyze \
  -F "file=@song.mp3" \
  -F "complexity=0.8"
```

Parameters:
- `file`: MP3 or WAV audio file (required)
- `complexity`: Pattern complexity between 0.0 and 1.0 (optional, default: 0.5)

### Configuration

Server configuration can be modified through environment variables:

```bash
RHYTHMIX_PORT=3000                  # Server port
RHYTHMIX_HOST=127.0.0.1            # Server host
RHYTHMIX_MAX_FILE_SIZE=10485760    # Maximum file size (10MB)
RHYTHMIX_THREADS=4                 # Analysis thread pool size
```

## Performance Considerations

- Audio analysis is CPU-intensive; recommended minimum 2 cores
- Memory usage scales with audio file length
- Pattern generation is optimized for real-time performance
- Caching system for previously analyzed files

## Testing

Run the test suite:
```bash
cargo test --all
```

Integration tests cover:
- Audio file processing
- Pattern generation accuracy
- API endpoints
- Error handling
- Edge cases

## Contributing

1. Fork the repository
2. Create a feature branch
3. Commit your changes
4. Push to the branch
5. Create a Pull Request

## License

MIT License - See LICENSE file for details

## Dependencies

Core dependencies:
- `hyper`: Async HTTP server
- `rodio`: Audio decoding
- `rustfft`: Fast Fourier Transform
- `serde`: Serialization
- `tokio`: Async runtime
- `crossbeam`: Thread pool management

## Roadmap

- [ ] Additional note types:
  - [ ] Hold notes - require holding for duration
  - [ ] Slide notes - directional swipes
  - [ ] Multi-tap notes - hit multiple lanes simultaneously
- [ ] Section-based intensity markers
- [ ] Additional audio format support (FLAC, OGG)
- [ ] Machine learning-based note placement
- [ ] Real-time WebSocket API for live note generation
- [ ] Performance profiling and optimization
- [ ] Beat markers for enhanced visual feedback