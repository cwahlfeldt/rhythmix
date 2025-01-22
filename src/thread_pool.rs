use crate::error::{Result, RhythmixError};
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

    /// Executes a task in the thread pool
    ///
    /// # Arguments
    /// * `f` - The task to execute
    ///
    /// # Errors
    /// Returns an error if the task cannot be sent to the worker threads
    pub fn execute<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce() -> Result<()> + Send + 'static,
    {
        self.sender.send(Box::new(f)).map_err(|e| {
            RhythmixError::ThreadPool(format!("Failed to send task to thread pool: {}", e))
        })?;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn test_thread_pool_creation() {
        assert!(ThreadPool::new(0).is_err());
        assert!(ThreadPool::new(1).is_ok());
    }

    #[test]
    fn test_task_execution() {
        let pool = ThreadPool::new(2).unwrap();
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();

        pool.execute(move || {
            let mut num = counter_clone.lock().unwrap();
            *num += 1;
            Ok(())
        })
        .unwrap();

        // Give some time for the task to complete
        thread::sleep(Duration::from_millis(100));
        assert_eq!(*counter.lock().unwrap(), 1);
    }

    #[test]
    fn test_multiple_tasks() {
        let pool = ThreadPool::new(4).unwrap();
        let counter = Arc::new(Mutex::new(0));
        let num_tasks = 10;

        for _ in 0..num_tasks {
            let counter_clone = counter.clone();
            pool.execute(move || {
                let mut num = counter_clone.lock().unwrap();
                *num += 1;
                Ok(())
            })
            .unwrap();
        }

        // Give some time for all tasks to complete
        thread::sleep(Duration::from_millis(200));
        assert_eq!(*counter.lock().unwrap(), num_tasks);
    }
}
