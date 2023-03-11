use local_sync::mpsc::unbounded::Tx;
use local_sync::oneshot::{channel, Sender};
use std::cell::RefCell;
use std::rc::Rc;
use std::{net::SocketAddr, str::FromStr, time::Instant};

use retty::bootstrap::{BootstrapClientUdp, BootstrapServerUdp};
use retty::channel::{
    Handler, InboundContext, InboundHandler, OutboundContext, OutboundHandler, Pipeline,
};
use retty::codec::{
    byte_to_message_decoder::{LineBasedFrameDecoder, TaggedByteToMessageCodec, TerminatorType},
    string_codec::{TaggedString, TaggedStringCodec},
};
use retty::transport::{AsyncTransport, TaggedBytesMut, TransportContext};

////////////////////////////////////////////////////////////////////////////////////////////////////

struct EchoDecoder {
    is_server: bool,
    tx: Rc<RefCell<Option<Sender<()>>>>,
    count: Rc<RefCell<usize>>,
}
struct EchoEncoder;
struct EchoHandler {
    decoder: EchoDecoder,
    encoder: EchoEncoder,
}

impl EchoHandler {
    fn new(
        is_server: bool,
        tx: Rc<RefCell<Option<Sender<()>>>>,
        count: Rc<RefCell<usize>>,
    ) -> Self {
        EchoHandler {
            decoder: EchoDecoder {
                is_server,
                tx,
                count,
            },
            encoder: EchoEncoder,
        }
    }
}

impl InboundHandler for EchoDecoder {
    type Rin = TaggedString;
    type Rout = Self::Rin;

    fn read(&mut self, ctx: &InboundContext<Self::Rin, Self::Rout>, msg: Self::Rin) {
        {
            let mut count = self.count.borrow_mut();
            *count += 1;
        }

        if self.is_server {
            ctx.fire_write(TaggedString {
                now: Instant::now(),
                transport: msg.transport,
                message: format!("{}\r\n", msg.message),
            });
        }

        if msg.message == "bye" {
            let mut tx = self.tx.borrow_mut();
            if let Some(tx) = tx.take() {
                let _ = tx.send(());
            }
        }
    }
    fn poll_timeout(&mut self, _ctx: &InboundContext<Self::Rin, Self::Rout>, _eto: &mut Instant) {
        //last handler, no need to fire_poll_timeout
    }
}

impl OutboundHandler for EchoEncoder {
    type Win = TaggedString;
    type Wout = Self::Win;

    fn write(&mut self, ctx: &OutboundContext<Self::Win, Self::Wout>, msg: Self::Win) {
        ctx.fire_write(msg);
    }
}

impl Handler for EchoHandler {
    type Rin = TaggedString;
    type Rout = Self::Rin;
    type Win = TaggedString;
    type Wout = Self::Win;

    fn name(&self) -> &str {
        "EchoHandler"
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

#[cfg(unix)]
#[monoio::test(timer_enabled = true)]
async fn test_echo_udp() {
    const ITER: usize = 1024;

    let (tx, rx) = channel();

    let server_count = Rc::new(RefCell::new(0));
    let server_count_clone = server_count.clone();
    let (server_done_tx, server_done_rx) = channel();
    let server_done_tx = Rc::new(RefCell::new(Some(server_done_tx)));

    let mut server = BootstrapServerUdp::new();
    server.pipeline(Box::new(move |writer: Tx<TaggedBytesMut>| {
        let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

        let async_transport_handler = AsyncTransport::new(writer);
        let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
            LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
        ));
        let string_codec_handler = TaggedStringCodec::new();
        let echo_handler = EchoHandler::new(
            true,
            Rc::clone(&server_done_tx),
            Rc::clone(&server_count_clone),
        );

        pipeline.add_back(async_transport_handler);
        pipeline.add_back(line_based_frame_decoder_handler);
        pipeline.add_back(string_codec_handler);
        pipeline.add_back(echo_handler);
        pipeline.finalize()
    }));

    let server_addr = server.bind("127.0.0.1:0").unwrap();

    monoio::spawn(async move {
        let client_count = Rc::new(RefCell::new(0));
        let client_count_clone = client_count.clone();
        let (client_done_tx, client_done_rx) = channel();
        let client_done_tx = Rc::new(RefCell::new(Some(client_done_tx)));

        let mut client = BootstrapClientUdp::new();
        client.pipeline(Box::new(move |writer: Tx<TaggedBytesMut>| {
            let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();

            let async_transport_handler = AsyncTransport::new(writer);
            let line_based_frame_decoder_handler = TaggedByteToMessageCodec::new(Box::new(
                LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH),
            ));
            let string_codec_handler = TaggedStringCodec::new();
            let echo_handler = EchoHandler::new(
                false,
                Rc::clone(&client_done_tx),
                Rc::clone(&client_count_clone),
            );

            pipeline.add_back(async_transport_handler);
            pipeline.add_back(line_based_frame_decoder_handler);
            pipeline.add_back(string_codec_handler);
            pipeline.add_back(echo_handler);
            pipeline.finalize()
        }));

        let client_addr = SocketAddr::from_str("127.0.0.1:0").unwrap();

        client.bind(client_addr).unwrap();
        let pipeline = client.connect(server_addr).await.unwrap();

        let message = format!("hello world\r\n");

        for _ in 0..ITER {
            // write
            pipeline.write(TaggedString {
                now: Instant::now(),
                transport: TransportContext {
                    local_addr: client_addr,
                    peer_addr: server_addr,
                    ecn: None,
                },
                message: message.clone(),
            });
        }
        pipeline.write(TaggedString {
            now: Instant::now(),
            transport: TransportContext {
                local_addr: client_addr,
                peer_addr: server_addr,
                ecn: None,
            },
            message: format!("bye\r\n"),
        });

        assert!(client_done_rx.await.is_ok());

        assert!(tx.send(client_count).is_ok());
    });

    assert!(server_done_rx.await.is_ok());

    let client_count = rx.await.unwrap();
    let (client_count, server_count) = (client_count.borrow(), server_count.borrow());
    assert_eq!(*client_count, *server_count);
    assert_eq!(ITER + 1, *client_count)
}