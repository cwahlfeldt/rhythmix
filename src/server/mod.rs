use crate::common::{Result, RhythmixError};
mod handlers;
mod thread_pool;

pub use handlers::{convert_error, handle_request};
use hyper::service::{make_service_fn, service_fn};
use hyper::Server;
use log::info;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
pub use thread_pool::ThreadPool;

/// Configuration for the HTTP server
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Host address to bind to
    pub host: String,
    /// Port to listen on
    pub port: u16,
    /// Number of threads in the processing pool
    pub thread_pool_size: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3001,
            thread_pool_size: 4,
        }
    }
}

#[derive(Clone)]
/// HTTP server for handling rhythm game pattern generation requests
pub struct RhythmixServer {
    config: ServerConfig,
    thread_pool: Arc<ThreadPool>,
}

impl RhythmixServer {
    /// Creates a new RhythmixServer with the specified configuration
    ///
    /// # Arguments
    /// * `config` - Server configuration
    pub fn new(config: ServerConfig) -> Result<Self> {
        let thread_pool = ThreadPool::new(config.thread_pool_size)?;

        Ok(Self {
            config,
            thread_pool: Arc::new(thread_pool),
        })
    }

    /// Starts the server and begins listening for requests
    pub async fn run(&self) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| RhythmixError::Server(format!("Invalid server address: {}", e)))?;

        let thread_pool = self.thread_pool.clone();

        // Create service handler
        let make_svc = make_service_fn(move |_conn| {
            let thread_pool = thread_pool.clone();
            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    let thread_pool = thread_pool.clone();
                    async move {
                        Ok::<_, Infallible>(
                            handle_request(req, thread_pool)
                                .await
                                .unwrap_or_else(convert_error),
                        )
                    }
                }))
            }
        });

        let server = Server::bind(&addr).serve(make_svc);

        info!("Server running on http://{}", addr);

        // Start the server
        server
            .await
            .map_err(|e| RhythmixError::Server(format!("Server error: {}", e)))?;

        Ok(())
    }
}

/// Loads server configuration from environment variables
pub fn load_config_from_env() -> ServerConfig {
    use std::env;

    ServerConfig {
        host: env::var("RHYTHMIX_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
        port: env::var("RHYTHMIX_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000),
        thread_pool_size: env::var("RHYTHMIX_THREADS")
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or(4),
    }
}
