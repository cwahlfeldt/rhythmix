use crate::audio_analyzer::{AnalysisConfig, AudioAnalyzer};
use crate::audio_decoder::AudioDecoder;
use crate::error::{helpers, Result, RhythmixError};
use crate::pattern_generator::{GeneratorConfig, PatternGenerator};
use crate::thread_pool::ThreadPool;
use bytes::Bytes;
use hyper::{Body, Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;

/// Maximum file size (10MB)
const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Request parameters for pattern generation
#[derive(Debug, Deserialize)]
pub struct PatternRequest {
    /// Desired pattern complexity (0.0 to 1.0)
    complexity: Option<f64>,
}

/// Response for pattern generation
#[derive(Debug, Serialize)]
pub struct PatternResponse {
    /// Success status
    success: bool,
    /// Pattern data if successful
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Handles incoming HTTP requests
pub async fn handle_request(
    req: Request<Body>,
    thread_pool: Arc<ThreadPool>,
) -> Result<Response<Body>> {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/analyze") => handle_analyze(req, thread_pool).await,
        (&Method::GET, "/health") => handle_health_check().await,
        _ => handle_not_found().await,
    }
}

/// Handles the /analyze endpoint for audio file processing
async fn handle_analyze(
    mut req: Request<Body>,
    thread_pool: Arc<ThreadPool>,
) -> Result<Response<Body>> {
    // Parse multipart form data
    let boundary = get_boundary(req.headers())?;
    let body_bytes = hyper::body::to_bytes(req.body_mut()).await.map_err(|e| {
        RhythmixError::InvalidRequest(format!("Failed to read request body: {}", e))
    })?;

    let (file_data, complexity) = parse_multipart(&body_bytes, &boundary)?;

    // Validate file size
    helpers::validate_file_size(file_data.len(), MAX_FILE_SIZE)?;

    // Process audio file
    let pattern_data = process_audio(file_data, complexity, thread_pool).await?;

    // Construct success response
    let response = PatternResponse {
        success: true,
        data: Some(serde_json::to_value(pattern_data)?),
        error: None,
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&response)?))?)
}

/// Handles health check requests
async fn handle_health_check() -> Result<Response<Body>> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from("OK"))?)
}

/// Handles 404 Not Found responses
async fn handle_not_found() -> Result<Response<Body>> {
    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))?)
}

/// Extracts the boundary from multipart form data
fn get_boundary(headers: &hyper::HeaderMap) -> Result<String> {
    let content_type = headers
        .get("content-type")
        .ok_or_else(|| RhythmixError::InvalidRequest("Missing content-type header".into()))?
        .to_str()
        .map_err(|_| RhythmixError::InvalidRequest("Invalid content-type header".into()))?;

    let boundary = content_type
        .split("boundary=")
        .nth(1)
        .ok_or_else(|| RhythmixError::InvalidRequest("Missing boundary in content-type".into()))?;

    Ok(boundary.to_string())
}

/// Parses multipart form data
fn parse_multipart(bytes: &[u8], boundary: &str) -> Result<(Vec<u8>, Option<f64>)> {
    let mut file_data = None;
    let mut complexity = None;

    // Simple multipart parser
    let parts: Vec<&[u8]> = bytes
        .split(|b| *b == b'\r' || *b == b'\n')
        .filter(|part| !part.is_empty())
        .collect();

    let mut i = 0;
    while i < parts.len() {
        let part = parts[i];
        if part.starts_with(b"--") && i + 3 < parts.len() {
            let content_disposition = String::from_utf8_lossy(parts[i + 1]);
            if content_disposition.contains("name=\"file\"") {
                file_data = Some(parts[i + 3].to_vec());
            } else if content_disposition.contains("name=\"complexity\"") {
                let value = String::from_utf8_lossy(parts[i + 3]);
                complexity = Some(value.parse::<f64>().map_err(|_| {
                    RhythmixError::InvalidRequest("Invalid complexity value".into())
                })?);
                helpers::validate_pattern_config(complexity.unwrap())?;
            }
            i += 4;
        } else {
            i += 1;
        }
    }

    let file_data = file_data
        .ok_or_else(|| RhythmixError::InvalidRequest("No file found in request".into()))?;

    Ok((file_data, complexity))
}

/// Processes audio data and generates pattern
async fn process_audio(
    file_data: Vec<u8>,
    complexity: Option<f64>,
    thread_pool: Arc<ThreadPool>,
) -> Result<crate::types::PatternData> {
    // Decode audio
    let decoder = AudioDecoder::from_bytes(file_data)?;
    let sample_rate = decoder.sample_rate();
    let mono_samples = decoder.to_mono();

    // Configure analysis
    let analysis_config = AnalysisConfig::default();
    let mut analyzer = AudioAnalyzer::new(analysis_config, sample_rate)?;

    // Process audio in chunks
    let chunk_size = 1024;
    for chunk in mono_samples.chunks(chunk_size) {
        analyzer.process_chunk(chunk)?;
    }

    // Get analysis results
    let analysis_results = analyzer.get_results();

    // Configure pattern generator
    let mut generator_config = GeneratorConfig::default();
    if let Some(c) = complexity {
        generator_config.difficulty.base_level = c;
    }

    // Generate pattern
    let mut generator = PatternGenerator::new(generator_config, analysis_results.bpm)?;
    let pattern_data = generator.generate_pattern(&analysis_results)?;

    Ok(pattern_data)
}

/// Converts errors to HTTP responses
pub fn convert_error(error: RhythmixError) -> Response<Body> {
    let status = match &error {
        RhythmixError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
        RhythmixError::FileTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
        RhythmixError::UnsupportedFileType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };

    let response = PatternResponse {
        success: false,
        data: None,
        error: Some(error.to_string()),
    };

    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&response).unwrap_or_else(
            |_| r#"{"success":false,"error":"Failed to serialize error response"}"#.to_string(),
        )))
        .unwrap_or_else(|_| Response::new(Body::from("Internal Server Error")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::header::HeaderValue;

    #[test]
    fn test_boundary_extraction() {
        let mut headers = hyper::HeaderMap::new();
        headers.insert(
            "content-type",
            HeaderValue::from_static("multipart/form-data; boundary=test123"),
        );

        let boundary = get_boundary(&headers).unwrap();
        assert_eq!(boundary, "test123");
    }

    #[test]
    fn test_error_conversion() {
        let error = RhythmixError::FileTooLarge {
            max_size: 1000,
            actual_size: 2000,
        };
        let response = convert_error(error);
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
