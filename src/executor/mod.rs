//! Async executors.

#[cfg(feature = "tokio")]
mod tokio;

use std::{
    future::Future,
    io::Result,
    thread::{JoinHandle},
};

/// An executor that provides thread local and blocking capabilities
///
/// Since there are many async runtimes, and there is no standard interfaces to them, you may
/// implement your own version of the trait. It should be simple enough, by relying on the underlying
/// tokio/smol/std/gloom implementations
pub trait LocalExecutor: Clone {
    /// Names the thread-to-be. Currently, the name is used for identification only in panic messages.
    fn name(&self) -> &str;

    /// Runs the local executor on the current thread until the given future completes.
    fn block_on<T>(&self, f: impl Future<Output = T>) -> T;

    /// Spawns a thread to run the local executor until the given future completes.
    fn spawn<G, F, T>(&self, fut_gen: G) -> Result<JoinHandle<T>>
    where
        G: FnOnce() -> F + Send + 'static,
        F: Future<Output = T> + 'static,
        T: Send + 'static;

    /// Spawns a thread to run the local executor until the given future completes.
    fn spawn_local<F, T>(&self, fut_gen: F) -> Result<JoinHandle<T>>
        where
            F: Future<Output = T> + 'static,
            T: Send + 'static;
}
