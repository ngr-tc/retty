use async_trait::async_trait;
use clap::Parser;
use std::io::stdin;
use std::io::Write;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use retty::bootstrap::BootstrapUdpEcnClient;
use retty::channel::{
    Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler, Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::{LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType},
    string_codec::{TaggedString, TaggedStringCodec},
};
use retty::runtime::default_runtime;
use retty::transport::{
    AsyncTransportUdpEcn, AsyncTransportWrite, TaggedBytesMut, TransportContext,
};

////////////////////////////////////////////////////////////////////////////////////////////////////

struct ChatDecoder;
struct ChatEncoder;
struct ChatHandler {
    decoder: ChatDecoder,
    encoder: ChatEncoder,
}

impl ChatHandler {
    fn new() -> Self {
        ChatHandler {
            decoder: ChatDecoder,
            encoder: ChatEncoder,
        }
    }
}

#[async_trait]
impl InboundHandler for ChatDecoder {
    type Rin = TaggedString;
    type Rout = Self::Rin;

    async fn read(&mut self, _ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        println!(
            "received back: {} from {:?} with ECN {:?}",
            msg.message, msg.transport.peer_addr, msg.transport.ecn
        );
    }
}

#[async_trait]
impl OutboundHandler for ChatEncoder {
    type Win = TaggedString;
    type Wout = Self::Win;

    async fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        ctx.fire_write(msg).await;
    }
}

impl Handler for ChatHandler {
    type Rin = TaggedString;
    type Rout = Self::Rin;
    type Win = TaggedString;
    type Wout = Self::Win;

    fn name(&self) -> &str {
        "ChatHandler"
    }

    fn split(
        self,
    ) -> (
        Box<dyn InboundHandler<Rin = Self::Rin, Rout = Self::Rout>>,
        Box<dyn OutboundHandler<Win = Self::Win, Wout = Self::Wout>>,
    ) {
        (Box::new(self.decoder), Box::new(self.encoder))
    }
}

#[derive(Parser)]
#[command(name = "Chat UDP Client with ECN")]
#[command(author = "Rusty Rain <y@liu.mx>")]
#[command(version = "0.1.0")]
#[command(about = "An example of chat upd client with ECN", long_about = None)]
struct Cli {
    #[arg(short, long)]
    debug: bool,
    #[arg(long, default_value_t = format!("0.0.0.0"))]
    host: String,
    #[arg(long, default_value_t = 8080)]
    port: u16,
    #[arg(long, default_value_t = format!("INFO"))]
    log_level: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let host = cli.host;
    let port = cli.port;
    let log_level = log::LevelFilter::from_str(&cli.log_level)?;
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
            .filter(None, log_level)
            .init();
    }

    println!("Connecting {}:{}...", host, port);

    let transport = TransportContext {
        local_addr: SocketAddr::from_str("0.0.0.0:0")?,
        peer_addr: Some(SocketAddr::from_str(&format!("{}:{}", host, port))?),
        ecn: None,
    };

    let mut bootstrap = BootstrapUdpEcnClient::new(default_runtime().unwrap());
    bootstrap.pipeline(Box::new(
        move |sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            Box::pin(async move {
                let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

                let async_transport_handler = AsyncTransportUdpEcn::new(sock);
                let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
                    LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
                ));
                let string_codec_handler = TaggedStringCodec::new();
                let chat_handler = ChatHandler::new();

                pipeline.add_back(async_transport_handler).await;
                pipeline.add_back(line_based_frame_decoder_handler).await;
                pipeline.add_back(string_codec_handler).await;
                pipeline.add_back(chat_handler).await;
                pipeline.finalize().await;

                Arc::new(pipeline)
            })
        },
    ));

    bootstrap.bind(transport.local_addr).await?;

    let pipeline = bootstrap
        .connect(transport.peer_addr.as_ref().unwrap())
        .await?;

    println!("Enter bye to stop");
    let mut buffer = String::new();
    while stdin().read_line(&mut buffer).is_ok() {
        match buffer.trim_end() {
            "" => break,
            line => {
                pipeline
                    .write(TaggedString {
                        now: Instant::now(),
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

    bootstrap.stop().await;

    Ok(())
}
