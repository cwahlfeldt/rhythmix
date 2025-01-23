mod audio_analyzer;
mod audio_decoder;
mod difficulty;
mod error;
mod fft_processor;
mod handlers;
mod onset_detector;
mod pattern_generator;
mod pattern_types;
mod section_detector;
mod server;
mod thread_pool;
mod types;

use crate::error::Result;
use crate::server::{load_config_from_env, RhythmixServer};
use log::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("Starting Rhythmix server...");

    // Load configuration from environment
    let config = load_config_from_env();
    info!("Server configuration loaded");
    info!("Host: {}", config.host);
    info!("Port: {}", config.port);
    info!("Thread pool size: {}", config.thread_pool_size);

    // Create and run server
    let server = match RhythmixServer::new(config) {
        Ok(server) => server,
        Err(e) => {
            error!("Failed to create server: {}", e);
            return Err(e);
        }
    };

    info!("Server initialized successfully");

    // Run the server
    if let Err(e) = server.run().await {
        error!("Server error: {}", e);
        return Err(e);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_env_configuration() {
        // Set test environment variables
        env::set_var("RHYTHMIX_HOST", "127.0.0.1");
        env::set_var("RHYTHMIX_PORT", "3000");
        env::set_var("RHYTHMIX_THREADS", "4");

        let config = load_config_from_env();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert_eq!(config.thread_pool_size, 4);
    }
}
