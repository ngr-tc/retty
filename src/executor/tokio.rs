use std::future::Future;
use std::thread::JoinHandle;
use crate::executor::LocalExecutor;

#[derive(Clone)]
pub struct TokioLocalExecutor {

}

pub fn new() -> TokioLocalExecutor {
    TokioLocalExecutor {

    }
}

impl LocalExecutor for TokioLocalExecutor {
    fn name(&self) -> &str {
        "retty-tokio-local-executor"
    }

    fn block_on<T>(&self, f: impl Future<Output=T>) -> T {
        tokio::runtime::Handle::current();
    }

    fn spawn<G, F, T>(&self, fut_gen: G) -> std::io::Result<JoinHandle<T>> where G: FnOnce() -> F + Send + 'static, F: Future<Output=T> + 'static, T: Send + 'static {
        todo!()
    }

    fn spawn_local<F, T>(&self, fut_gen: F) -> std::io::Result<JoinHandle<T>> where F: Future<Output=T> + 'static, T: Send + 'static {
        todo!()
    }
}