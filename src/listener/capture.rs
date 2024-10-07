use pcap::{Active, Capture, Device, PacketHeader};
use std::error::Error;
use log::{error, info};
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver, unbounded_channel};
use tokio::task;


pub struct PacketCapturer {
    cap: Capture<Active>,
    sender: UnboundedSender<OwnedPacket>,
}

#[derive(Clone, Debug)]
pub struct OwnedPacket {
    pub header: PacketHeader,
    pub data: Vec<u8>,
}

impl PacketCapturer {
    /*
     * Create a new PacketCapturer instance
     */
    pub fn new() -> Result<(Self, UnboundedReceiver<OwnedPacket>), Box<dyn Error>> {
        let device = Device::lookup()?
            .ok_or("No device available for capture")?;
        info!("Using device: {}", device.name);

        let cap = Capture::from_device(device)?
            .promisc(true)
            .immediate_mode(true)
            .timeout(1000) // Timeout in milliseconds
            .open()?;

        let (sender, receiver) = unbounded_channel();

        Ok((PacketCapturer { cap, sender }, receiver))
    }

    /*
     * Start the asynchronous packet capturing loop
     */
    pub fn start_capture_loop(mut self) {
        // Clone the sender to move into the thread
        let sender = self.sender.clone();
        // Capture needs to be in a blocking task since pcap::Capture is blocking
        task::spawn_blocking(move || {
            loop {
                match self.cap.next_packet() {
                    Ok(packet) => {
                        let packet = OwnedPacket {
                            header: *packet.header,
                            data: packet.data.to_vec(),
                        };
                        if sender.send(packet).is_err() {
                            // Receiver has been dropped
                            error!("Receiver dropped. Stopping packet capture.");
                            break;
                        }
                    },
                    Err(e) => {
                        error!("Error capturing packet: {}", e);
                        continue;
                    }
                }
            }
        });
    }
}

/*
 * Capture packets and log them
 */
pub async fn capture_packets() -> Result<(), Box<dyn Error>> {

    info!("Starting packet capture");
    let (pcap, mut receiver)
        = PacketCapturer::new()?;

    pcap.start_capture_loop();

    while let Some(packet) = receiver.recv().await {
        info!("Received packet: {:?}", packet.header);
    }

    Ok(())
}