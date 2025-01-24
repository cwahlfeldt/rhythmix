use std::io;
use thiserror::Error;

/// Custom error types for the Rhythmix application
#[derive(Error, Debug)]
pub enum RhythmixError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Audio decoding error: {0}")]
    AudioDecoding(String),

    #[error("FFT processing error: {0}")]
    FftProcessing(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("File too large: maximum size is {max_size} bytes, got {actual_size} bytes")]
    FileTooLarge { max_size: usize, actual_size: usize },

    #[error("Unsupported file type: {0}")]
    UnsupportedFileType(String),

    #[error("Thread pool error: {0}")]
    ThreadPool(String),

    #[error("Server error: {0}")]
    Server(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] hyper::http::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias for Rhythmix operations
pub type Result<T> = std::result::Result<T, RhythmixError>;

impl From<&str> for RhythmixError {
    fn from(error: &str) -> Self {
        RhythmixError::Internal(error.to_string())
    }
}

impl From<String> for RhythmixError {
    fn from(error: String) -> Self {
        RhythmixError::Internal(error)
    }
}

/// Helper functions for error handling
pub mod helpers {
    use super::*;

    /// Validates an audio file's size against the maximum allowed size
    pub fn validate_file_size(size: usize, max_size: usize) -> Result<()> {
        if size > max_size {
            return Err(RhythmixError::FileTooLarge {
                max_size,
                actual_size: size,
            });
        }
        Ok(())
    }

    /// Validates pattern generation parameters
    pub fn validate_pattern_config(complexity: f64) -> Result<()> {
        if !(0.0..=1.0).contains(&complexity) {
            return Err(RhythmixError::InvalidConfig(format!(
                "Complexity must be between 0.0 and 1.0, got {}",
                complexity
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_file_size() {
        assert!(helpers::validate_file_size(1000, 2000).is_ok());
        let err = helpers::validate_file_size(2000, 1000).unwrap_err();
        match err {
            RhythmixError::FileTooLarge {
                max_size,
                actual_size,
            } => {
                assert_eq!(max_size, 1000);
                assert_eq!(actual_size, 2000);
            }
            _ => panic!("Expected FileTooLarge error"),
        }
    }

    #[test]
    fn test_validate_pattern_config() {
        assert!(helpers::validate_pattern_config(0.5).is_ok());
        assert!(helpers::validate_pattern_config(0.0).is_ok());
        assert!(helpers::validate_pattern_config(1.0).is_ok());
        assert!(helpers::validate_pattern_config(-0.1).is_err());
        assert!(helpers::validate_pattern_config(1.1).is_err());
    }
}
