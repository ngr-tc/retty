use super::*;

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for TCP clients.
pub struct BootstrapTcpClient<W, E: LocalExecutor + 'static> {
    bootstrap_tcp: BootstrapTcp<W, E>,
}

impl<W: 'static, E: LocalExecutor + 'static> BootstrapTcpClient<W, E> {
    /// Creates a new BootstrapTcpClient
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

    /// Creates pipeline instances from when calling [BootstrapTcpClient::connect].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.bootstrap_tcp.pipeline(pipeline_factory_fn);
        self
    }

    /// Connects to the remote peer
    pub async fn connect<A: AsyncToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> Result<Rc<dyn OutboundPipeline<TaggedBytesMut, W>>, Error> {
        self.bootstrap_tcp.connect(addr).await
    }

    /// Stops the client
    pub async fn stop(&self) {
        self.bootstrap_tcp.stop().await
    }

    /// Waits for stop of the client
    pub async fn wait_for_stop(&self) {
        self.bootstrap_tcp.wait_for_stop().await
    }

    /// Gracefully stop the client
    pub async fn graceful_stop(&self) {
        self.bootstrap_tcp.graceful_stop().await
    }
}
