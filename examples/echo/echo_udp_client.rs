use async_trait::async_trait;
use clap::Parser;
use std::io::stdin;
use std::io::Write;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use retty::bootstrap::bootstrap_udp_client::BootstrapUdpClient;
use retty::channel::{
    handler::{
        Handler, InboundHandler, InboundHandlerContext, InboundHandlerGeneric, OutboundHandler,
        OutboundHandlerGeneric,
    },
    pipeline::Pipeline,
};
use retty::codec::byte_to_message_decoder::{
    line_based_frame_decoder::{LineBasedFrameDecoder, TerminatorType},
    tagged::TaggedByteToMessageCodec,
};
use retty::codec::string_codec::tagged::{TaggedString, TaggedStringCodec};
use retty::error::Error;
use retty::runtime::{default_runtime, sync::Mutex};
use retty::transport::async_transport_udp::AsyncTransportUdp;
use retty::transport::{AsyncTransportWrite, TransportContext};

////////////////////////////////////////////////////////////////////////////////////////////////////

struct TaggedEchoDecoder;
struct TaggedEchoEncoder;
struct TaggedEchoHandler {
    decoder: TaggedEchoDecoder,
    encoder: TaggedEchoEncoder,
}

impl TaggedEchoHandler {
    fn new() -> Self {
        TaggedEchoHandler {
            decoder: TaggedEchoDecoder,
            encoder: TaggedEchoEncoder,
        }
    }
}

#[async_trait]
impl InboundHandlerGeneric<TaggedString> for TaggedEchoDecoder {
    async fn read_generic(&mut self, _ctx: &mut InboundHandlerContext, msg: &mut TaggedString) {
        println!(
            "received back: {} from {:?}",
            msg.message, msg.transport.peer_addr
        );
    }
}

impl OutboundHandlerGeneric<TaggedString> for TaggedEchoEncoder {}

impl Handler for TaggedEchoHandler {
    fn id(&self) -> String {
        "TaggedEcho Handler".to_string()
    }

    fn split(
        self,
    ) -> (
        Arc<Mutex<dyn InboundHandler>>,
        Arc<Mutex<dyn OutboundHandler>>,
    ) {
        let decoder: Box<dyn InboundHandlerGeneric<TaggedString>> = Box::new(self.decoder);
        let encoder: Box<dyn OutboundHandlerGeneric<TaggedString>> = Box::new(self.encoder);
        (Arc::new(Mutex::new(decoder)), Arc::new(Mutex::new(encoder)))
    }
}

#[derive(Parser)]
#[command(name = "Echo UDP Client")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
#[command(about = "An example of echo udp client", long_about = None)]
struct Cli {
    #[arg(short, long)]
    debug: bool,
    #[arg(long, default_value_t = format!("0.0.0.0"))]
    host: String,
    #[arg(long, default_value_t = 8080)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli = Cli::parse();
    let host = cli.host;
    let port = cli.port;
    if cli.debug {
        env_logger::Builder::new()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "{}:{} [{}] {} - {}",
                    record.file().unwrap_or("unknown"),
                    record.line().unwrap_or(0),
                    record.level(),
                    chrono::Local::now().format("%H:%M:%S.%6f"),
                    record.args()
                )
            })
            .filter(None, log::LevelFilter::Trace)
            .init();
    }

    println!("Connecting {}:{}...", host, port);

    let transport = TransportContext {
        local_addr: SocketAddr::from_str("0.0.0.0:0")?,
        peer_addr: Some(SocketAddr::from_str(&format!("{}:{}", host, port))?),
    };

    let mut client = BootstrapUdpClient::new(default_runtime().unwrap());
    client.pipeline(Box::new(
        move |sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            let mut pipeline = Pipeline::new(TransportContext {
                local_addr: sock.local_addr().unwrap(),
                peer_addr: sock.peer_addr().ok(),
            });

            let async_transport_handler = AsyncTransportUdp::new(sock);
            let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
                LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
            ));
            let string_codec_handler = TaggedStringCodec::new();
            let echo_handler = TaggedEchoHandler::new();

            pipeline.add_back(async_transport_handler);
            pipeline.add_back(line_based_frame_decoder_handler);
            pipeline.add_back(string_codec_handler);
            pipeline.add_back(echo_handler);

            Box::pin(async move { pipeline.finalize().await })
        },
    ));

    client.bind(transport.local_addr).await?;

    let pipeline = client
        .connect(transport.peer_addr.as_ref().unwrap())
        .await?;

    println!("Enter bye to stop");
    let mut buffer = String::new();
    while stdin().read_line(&mut buffer).is_ok() {
        match buffer.trim_end() {
            "" => break,
            line => {
                pipeline
                    .write(&mut TaggedString {
                        transport,
                        message: format!("{}\r\n", line),
                    })
                    .await;
                if line == "bye" {
                    pipeline.close().await;
                    break;
                }
            }
        };
        buffer.clear();
    }

    Ok(())
}
