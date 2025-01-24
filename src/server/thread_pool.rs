use crate::common::{Result, RhythmixError};
use crossbeam::channel::{bounded, Receiver, Sender};
use log::{debug, error};
use std::thread::{self, JoinHandle};

/// A task that can be executed by the thread pool
type Task = Box<dyn FnOnce() -> Result<()> + Send + 'static>;

/// A thread pool for executing audio processing tasks concurrently
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Sender<Task>,
}

impl ThreadPool {
    /// Creates a new ThreadPool with the specified number of threads
    ///
    /// # Arguments
    /// * `size` - The number of threads in the pool
    ///
    /// # Errors
    /// Returns an error if the size is 0
    pub fn new(size: usize) -> Result<ThreadPool> {
        if size == 0 {
            return Err(RhythmixError::ThreadPool(
                "Thread pool size must be greater than 0".into(),
            ));
        }

        let (sender, receiver) = bounded(size * 2);
        let receiver = std::sync::Arc::new(receiver);
        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, receiver.clone())?);
        }

        Ok(ThreadPool { workers, sender })
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        debug!("Shutting down thread pool");
        drop(self.sender.clone());

        for worker in &mut self.workers {
            debug!("Shutting down worker {}", worker.id);
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap_or_else(|e| {
                    error!("Error joining worker thread {}: {:?}", worker.id, e);
                });
            }
        }
    }
}

struct Worker {
    id: usize,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: std::sync::Arc<Receiver<Task>>) -> Result<Worker> {
        let thread = thread::Builder::new()
            .name(format!("worker-{}", id))
            .spawn(move || {
                debug!("Worker {} started", id);
                while let Ok(task) = receiver.recv() {
                    debug!("Worker {} received a task", id);
                    if let Err(e) = task() {
                        error!("Worker {} task error: {:?}", id, e);
                    }
                }
                debug!("Worker {} shutting down", id);
            })
            .map_err(|e| RhythmixError::ThreadPool(format!("Failed to spawn thread: {}", e)))?;

        Ok(Worker {
            id,
            thread: Some(thread),
        })
    }
}
