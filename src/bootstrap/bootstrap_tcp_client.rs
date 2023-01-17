use bytes::BytesMut;
use log::{trace, warn};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::bootstrap::{PipelineFactoryFn, MAX_DURATION};
use crate::channel::pipeline::PipelineContext;
use crate::error::Error;
use crate::runtime::{
    io::AsyncReadExt,
    net::{TcpStream, ToSocketAddrs},
    sleep, Runtime,
};

pub struct BootstrapTcpClient {
    pipeline_factory_fn: Option<Arc<PipelineFactoryFn>>,
    runtime: Arc<dyn Runtime>,
}

impl BootstrapTcpClient {
    pub fn new(runtime: Arc<dyn Runtime>) -> Self {
        Self {
            pipeline_factory_fn: None,
            runtime,
        }
    }

    pub fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn) -> &mut Self {
        self.pipeline_factory_fn = Some(Arc::new(Box::new(pipeline_factory_fn)));
        self
    }

    /// connect host:port
    pub async fn connect<A: ToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> Result<Arc<PipelineContext>, Error> {
        let socket = TcpStream::connect(addr).await?;

        #[cfg(feature = "runtime-tokio")]
        let (mut socket_rd, socket_wr) = socket.into_split();
        #[cfg(feature = "runtime-async-std")]
        let (mut socket_rd, socket_wr) = (socket.clone(), socket);

        let pipeline_factory_fn = Arc::clone(self.pipeline_factory_fn.as_ref().unwrap());
        let async_writer = Box::new(socket_wr);
        let pipeline_wr = Arc::new((pipeline_factory_fn)(async_writer).await);

        let pipeline = Arc::clone(&pipeline_wr);
        self.runtime.spawn(Box::pin(async move {
            let mut buf = vec![0u8; 8196];

            pipeline.transport_active().await;
            loop {
                let mut timeout = Instant::now() + Duration::from_secs(MAX_DURATION);
                pipeline.poll_timeout(&mut timeout).await;

                let timer = if let Some(duration) = timeout.checked_duration_since(Instant::now()) {
                    sleep(duration)
                } else {
                    sleep(Duration::from_secs(0))
                };
                tokio::pin!(timer);

                tokio::select! {
                    _ = timer.as_mut() => {
                        pipeline.read_timeout(Instant::now()).await;
                    }
                    res = socket_rd.read(&mut buf) => {
                        match res {
                            Ok(n) => {
                                if n == 0 {
                                    pipeline.read_eof().await;
                                    break;
                                }

                                trace!("pipeline recv {} bytes", n);
                                pipeline.read(&mut BytesMut::from(&buf[..n])).await;
                            }
                            Err(err) => {
                                warn!("TcpStream read error {}", err);
                                break;
                            }
                        };
                    }
                }
            }
            pipeline.transport_inactive().await;
        }));

        Ok(pipeline_wr)
    }
}
