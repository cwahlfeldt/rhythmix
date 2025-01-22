use crate::error::{Result, RhythmixError};
use crate::handlers::{convert_error, handle_request};
use crate::thread_pool::ThreadPool;
use hyper::service::{make_service_fn, service_fn};
use hyper::Server;
use log::info;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

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
            port: 3000,
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

    /// Gets a reference to the server configuration
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Gets a reference to the thread pool
    pub fn thread_pool(&self) -> &ThreadPool {
        &self.thread_pool
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

#[cfg(test)]
mod tests {
    use crate::error;

    use super::*;
    use hyper::{Body, Client, Method, Request};
    use std::time::Duration;
    use tokio::time::sleep;

    async fn start_test_server() -> RhythmixServer {
        let config = ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 0, // Let OS choose port
            thread_pool_size: 2,
        };

        let server = RhythmixServer::new(config).unwrap();
        let server_clone = server.clone();

        // Start server in background
        tokio::spawn(async move {
            if let Err(e) = server_clone.run().await {
                error!("Server error: {}", e);
            }
        });

        // Give server time to start
        sleep(Duration::from_millis(100)).await;

        server
    }

    #[tokio::test]
    async fn test_health_check() {
        let server = start_test_server().await;
        let client = Client::new();

        let addr = format!("http://{}:{}", server.config().host, server.config().port);

        let req = Request::builder()
            .method(Method::GET)
            .uri(format!("{}/health", addr))
            .body(Body::empty())
            .unwrap();

        let resp = client.request(req).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn test_not_found() {
        let server = start_test_server().await;
        let client = Client::new();

        let addr = format!("http://{}:{}", server.config().host, server.config().port);

        let req = Request::builder()
            .method(Method::GET)
            .uri(format!("{}/nonexistent", addr))
            .body(Body::empty())
            .unwrap();

        let resp = client.request(req).await.unwrap();
        assert_eq!(resp.status(), 404);
    }

    #[test]
    fn test_load_config_from_env() {
        std::env::set_var("RHYTHMIX_HOST", "0.0.0.0");
        std::env::set_var("RHYTHMIX_PORT", "8080");
        std::env::set_var("RHYTHMIX_THREADS", "8");

        let config = load_config_from_env();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert_eq!(config.thread_pool_size, 8);
    }
}
