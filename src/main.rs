use log::info;
use network_listener::listener::{capture::PacketCapturer, parser::Parser};
use network_listener::logging::logger;
use network_listener::probe::iperf::IperfServer;
use network_listener::IPERF3_PORT;
use std::error::Error;
use std::net::IpAddr;
use tokio::sync::mpsc::unbounded_channel;
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

pub enum EventMessage {
    DoIperf3(IpAddr),
    DoPing(IpAddr),
    DoExit,
    PausePCAP,
}

type Modules = (PacketCapturer, Parser, IperfServer);

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

    pub fn init_modules(&mut self) -> Result<Modules, Box<dyn Error>> {
        let (sender, receiver) = unbounded_channel();
        let iperf_sender = sender.clone();

        let (pcap, pcap_meta) = PacketCapturer::new(sender.clone())?;
        let parser = Parser::new(receiver, pcap_meta)?;

        let server = IperfServer::new(IPERF3_PORT, iperf_sender)?;
        Ok((pcap, parser, server))
    }

    pub fn start(&mut self) -> Result<(), Box<dyn Error>> {
        info!("Starting packet capture");

        let (pcap, parser, server) = self.init_modules()?;

        let cap_h = pcap.start_capture_loop();
        let parser_h = tokio::spawn(async move {parser.start().await});
        let server_h = tokio::spawn(async move {server.start().await});
        self.handles.push(cap_h);
        self.handles.push(parser_h);
        self.result_handles.push(server_h);
        Ok(())
    }

    pub async fn blocking_event_loop(mut self) -> Self {
        // Event loop
        loop {
            tokio::select! {
                Some(event) = self.event_receiver.recv() => match event {
                    EventMessage::DoIperf3(ip) => {
                        info!("Starting iperf3 to {}", ip);
                    }
                    EventMessage::DoPing(ip) => {
                        info!("Pinging {}", ip);
                    }
                    EventMessage::DoExit => {
                        info!("Exiting");
                        break;
                    }
                    EventMessage::PausePCAP => {
                        info!("Pausing PCAP");
                    }
                },
                _ = tokio::signal::ctrl_c() => {
                    info!("Received Ctrl-C");
                    break;
                },
                else => {
                    info!("Event channel closed");
                    break;
                }
            }
        };

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
    // let _ = tokio::spawn(network_listener::grafana::client::start_client());

    let mut netlistener = NetworkListener::new()?;
    netlistener.start()?;
    netlistener.blocking_event_loop().await.stop().await;
    Ok(())
}

