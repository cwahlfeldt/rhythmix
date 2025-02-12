use crate::audio::decoder::AudioDecoder;
use crate::audio::{AnalysisConfig, AudioAnalyzer};
use crate::common::{error::helpers, Result, RhythmixError};
use crate::pattern::generator::{GeneratorConfig, PatternGenerator};
use crate::server::thread_pool::ThreadPool;
use base64::Engine;
use hyper::{Body, Method, Request, Response, StatusCode};
use id3::{Tag, TagLike};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::Arc;

/// Maximum file size (10MB)
const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Request parameters for pattern generation
#[derive(Debug, Deserialize)]
pub struct PatternRequest {
    #[serde(default)]
    pub grid_division: Option<String>, // "whole", "half", "quarter", "eighth", "sixteenth"
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

    let (file_data, complexity, filename, grid_division) = parse_multipart(&body_bytes, &boundary)?;

    // Validate file size
    helpers::validate_file_size(file_data.len(), MAX_FILE_SIZE)?;

    // Process audio file
    let pattern_data = process_audio(file_data, complexity, filename, grid_division, thread_pool).await?;

    // Construct success response
    let response = PatternResponse {
        success: true,
        data: Some(serde_json::to_value(pattern_data)?),
        error: None,
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "POST, OPTIONS")
        .header("Access-Control-Allow-Headers", "Content-Type")
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
fn parse_multipart(bytes: &[u8], boundary: &str) -> Result<(Vec<u8>, Option<f64>, Option<String>, Option<String>)> {
    let mut file_data = None;
    let mut complexity = None;
    let mut filename = None;
    let mut grid_division = None;

    let full_boundary = format!("--{}", boundary);
    let boundary_bytes = full_boundary.as_bytes();
    let _start_idx = 0;

    // Find all boundary positions
    let mut boundary_positions: Vec<usize> = bytes
        .windows(boundary_bytes.len())
        .enumerate()
        .filter(|(_, window)| *window == boundary_bytes)
        .map(|(i, _)| i)
        .collect();

    // Add the final position
    boundary_positions.push(bytes.len());

    // Process each part
    for i in 0..boundary_positions.len() - 1 {
        let start = boundary_positions[i];
        let end = boundary_positions[i + 1];
        let part = &bytes[start..end];

        // Find the header end (double CRLF)
        if let Some(header_end) = find_double_crlf(part) {
            let headers = &part[..header_end];
            let content_start = header_end + 4; // Skip double CRLF
            let content = &part[content_start..];

            let headers_str = String::from_utf8_lossy(headers);

            if headers_str.contains("name=\"file\"") {
                // Extract filename if present
                if let Some(filename_start) = headers_str.find("filename=\"") {
                    if let Some(filename_end) = headers_str[filename_start + 10..].find('\"') {
                        filename = Some(
                            headers_str[filename_start + 10..filename_start + 10 + filename_end]
                                .to_string(),
                        );
                    }
                }

                // Remove trailing CRLF if present
                let content_len = if content.ends_with(b"\r\n") {
                    content.len() - 2
                } else {
                    content.len()
                };
                file_data = Some(content[..content_len].to_vec());
            } else if headers_str.contains("name=\"complexity\"") {
                let value = String::from_utf8_lossy(content).trim().to_string();
                if !value.is_empty() {
                    complexity = Some(value.parse::<f64>().map_err(|_| {
                        RhythmixError::InvalidRequest("Invalid complexity value".into())
                    })?);
                    helpers::validate_pattern_config(complexity.unwrap())?;
                }
            } else if headers_str.contains("name=\"grid_division\"") {
                let value = String::from_utf8_lossy(content).trim().to_string();
                if !value.is_empty() {
                    grid_division = Some(value);
                }
            }
        }
    }

    let file_data = file_data
        .ok_or_else(|| RhythmixError::InvalidRequest("No file found in request".into()))?;

    Ok((file_data, complexity, filename, grid_division))
}

/// Find position of double CRLF in bytes
fn find_double_crlf(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

/// Processes audio data and generates pattern
async fn process_audio(
    file_data: Vec<u8>,
    complexity: Option<f64>,
    filename: Option<String>,
    grid_division: Option<String>,
    _thread_pool: Arc<ThreadPool>,
) -> Result<crate::common::types::PatternData> {
    // Log file data size
    log::info!("Processing audio file of size {} bytes", file_data.len());

    // Create cursor and try to decode
    let cursor = Cursor::new(file_data.to_owned());
    let decoder = AudioDecoder::from_bytes(cursor.into_inner())?;
    let sample_rate = decoder.sample_rate();
    let mono_samples = decoder.to_mono();

    // Configure analysis
    let analysis_config = AnalysisConfig::default();
    // Use clone() to keep a copy of the config
    let chunk_size = analysis_config.onset_config.window_size;
    let mut analyzer = AudioAnalyzer::new(analysis_config, sample_rate)?;

    let mut start_idx = 0;
    while start_idx + chunk_size <= mono_samples.len() {
        let chunk = &mono_samples[start_idx..start_idx + chunk_size];
        analyzer.process_chunk(chunk)?;
        start_idx += chunk_size;
    }

    // Get analysis results
    let analysis_results = analyzer.get_results();

    // Configure pattern generator
    let mut generator_config = GeneratorConfig::default();
    if let Some(c) = complexity {
        generator_config.difficulty_config.base_level = c;
    }
    let tag_cursor = Cursor::new(&file_data);
    let title = Tag::read_from2(tag_cursor)
        .ok()
        .and_then(|tag| tag.title().map(|s| s.to_string()))
        .unwrap_or_else(|| filename.unwrap_or_else(|| "Unknown".to_string()));

    // Generate pattern
    // Create pattern generator
    let mut generator = PatternGenerator::new(
        generator_config,
        analysis_results.bpm,
        title,
    )?;

    // Set grid division if specified
    if let Some(division) = grid_division {
        use crate::pattern::timing::GridDivision;
        let division = match division.to_lowercase().as_str() {
            "doublebreve" => GridDivision::DoubleBreve,
            "breve" => GridDivision::Breve,
            "whole" => GridDivision::Whole,
            "half" => GridDivision::Half,
            "quarter" => GridDivision::Quarter,
            "eighth" => GridDivision::Eighth,
            "sixteenth" => GridDivision::Sixteenth,
            _ => GridDivision::Quarter, // Default to quarter notes
        };
        generator.set_grid_division(division);
    }

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
