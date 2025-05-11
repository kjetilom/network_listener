use log::info;
use network_listener::listener::{capture::PacketCapturer, parser::Parser};
use network_listener::logging::logger;
use network_listener::probe::iperf::IperfServer;
use network_listener::prost_net::bandwidth_client::ClientHandlerEvent;
use network_listener::proto_bw::DataMsg;
use network_listener::{prost_net, CapEvent, CONFIG, IPERF3_PORT};
use prost_net::bandwidth_client::ClientHandler;
use prost_net::bandwidth_server::BwServer;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::mpsc::{channel, unbounded_channel};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
pub type EventSender = tokio::sync::mpsc::UnboundedSender<EventMessage>;
pub type EventReceiver = tokio::sync::mpsc::UnboundedReceiver<EventMessage>;

// Struct representation of the crate.
pub struct NetworkListener {
    event_receiver: EventReceiver,
    _event_sender: EventSender,
    handles: Vec<JoinHandle<()>>,
    result_handles: Vec<JoinHandle<anyhow::Result<()>>>,
}

/// Enum representing events that can be sent to the main event loop.
/// The idea is to be able to pause and resume the packet capture to do
/// active measurements, but this is not implemented or used.
pub enum EventMessage {
    /// Pause the packet capture
    PausePCAP(tokio::time::Duration),
    /// Resume the packet capture
    ResumePCAP,
}

impl NetworkListener {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let (_event_sender, event_receiver) = unbounded_channel();
        Ok(Self {
            event_receiver,
            _event_sender,
            handles: vec![],
            result_handles: vec![],
        })
    }

    /// Start all the different tasks and components of the network listener.
    /// This includes the packet capture, parser, client handler, and server.
    ///
    /// It creates channels for communication between the components and
    /// dispatches the tasks to run concurrently.
    pub fn start(&mut self) -> Result<(), Box<dyn Error>> {
        info!("Starting packet capture");

        let (sender, receiver) = channel::<CapEvent>(1000);
        let (client_sender, client_receiver) = channel::<ClientHandlerEvent>(100);
        let (bw_message_bc, _bw_message_rx) = broadcast::channel::<DataMsg>(4);
        let bw_message_bc = Arc::new(bw_message_bc);

        let (pcap, pcap_meta) =
            PacketCapturer::new(sender.clone(), crate::CONFIG.client.iface.clone())?;
        let pcap_meta = Arc::new(pcap_meta);
        let (parser, ctx) = Parser::new(receiver, pcap_meta.clone(), client_sender)?;
        let client_handler = ClientHandler::new(ctx, client_receiver, sender.clone(), bw_message_bc.clone());
        let server = IperfServer::new(IPERF3_PORT, sender.clone())?;

        // Pass Arc reference to the bandwidth message channel
        let bw_server = BwServer::new(sender.clone(), pcap_meta.clone(), bw_message_bc.clone());

        let bw_client_h = client_handler.dispatch_client_handler();
        let cap_h = pcap.start_capture_loop();
        let parser_h = parser.dispatch_parser();
        let server_h = server.dispatch_server();
        let bw_server_h = bw_server.dispatch_server();
        //let pathload_h = network_listener::probe::pathload::dispatch_server();

        self.handles.push(parser_h);
        self.handles.push(bw_client_h);
        //self.handles.push(pathload_h);
        self.result_handles.push(cap_h);
        self.result_handles.push(server_h);
        self.result_handles.push(bw_server_h);
        Ok(())
    }

    pub async fn blocking_event_loop(mut self) -> Self {
        // Event loop
        loop {
            tokio::select! {
                Some(event) = self.event_receiver.recv() => match event {
                    EventMessage::PausePCAP(_) => {
                        info!("Not implemented (pause packet capture)");
                    },
                    EventMessage::ResumePCAP => {
                        info!("Not implemented (resume packet capture)");
                    },
                },
                _ = tokio::signal::ctrl_c() => {
                    info!("Received Ctrl-C, Stopping all tasks");
                    break;
                },
                else => {
                    info!("Event channel closed");
                    break;
                }
            }
        }

        self
    }

    pub async fn stop(self) {
        // Stop the parser
        for handle in self.handles {
            if handle.is_finished() {
                continue;
            }
            handle.abort();
        }
        for handle in self.result_handles {
            if handle.is_finished() {
                continue;
            }
            handle.abort();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    logger::setup_logging()?;
    let mut netlistener = NetworkListener::new()?;
    netlistener.start()?;
    // Start the core event loop, as of now it just blocks until Ctrl-C
    // is received, but it could be used to pause and resume the packet
    // capture.
    netlistener.blocking_event_loop().await.stop().await;

    // All tasks are stopped, return Ok(());
    Ok(())
}
