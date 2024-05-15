use super::*;

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for TCP servers.
pub struct BootstrapTcpServer<W, E: LocalExecutor + 'static> {
    bootstrap_tcp: BootstrapTcp<W, E>,
}

impl<W: 'static, E:LocalExecutor+'static> BootstrapTcpServer<W, E> {
    /// Creates a new BootstrapTcpServer
    pub fn new(e: E) -> Self {
        Self {
            bootstrap_tcp: BootstrapTcp::new(e),
        }
    }

    /// Sets max payload size, default is 2048 bytes
    pub fn max_payload_size(&mut self, max_payload_size: usize) -> &mut Self {
        self.bootstrap_tcp.max_payload_size(max_payload_size);
        self
    }

    /// Creates pipeline instances from when calling [BootstrapTcpServer::bind].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.bootstrap_tcp.pipeline(pipeline_factory_fn);
        self
    }

    /// Binds local address and port
    pub async fn bind<A: AsyncToSocketAddrs>(&self, addr: A) -> Result<SocketAddr, Error> {
        self.bootstrap_tcp.bind(addr).await
    }

    /// Stops the server
    pub async fn stop(&self) {
        self.bootstrap_tcp.stop().await
    }

    /// Waits for stop of the server
    pub async fn wait_for_stop(&self) {
        self.bootstrap_tcp.wait_for_stop().await
    }

    /// Gracefully stop the server
    pub async fn graceful_stop(&self) {
        self.bootstrap_tcp.graceful_stop().await
    }
}
