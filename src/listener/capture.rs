use log::{error, info};
use pcap::{Active, Capture, Device, Packet, PacketHeader};
use std::error::Error;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
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

impl<'a> From<Packet<'a>> for OwnedPacket {
    fn from(packet: Packet<'a>) -> Self {
        OwnedPacket {
            header: *packet.header,
            data: packet.data.to_vec(),
        }
    }
}

impl PacketCapturer {
    /**
     *  Create a new PacketCapturer instance
     */
    pub fn new() -> Result<(Self, UnboundedReceiver<OwnedPacket>), Box<dyn Error>> {
        let device = Device::lookup()?.ok_or("No device available for capture")?;
        info!("Using device: {}", device.name);

        let cap = Capture::from_device(device)?
            .promisc(true)
            .immediate_mode(true)
            .timeout(1000) // Timeout in milliseconds
            .open()?;

        let (sender, receiver) = unbounded_channel();

        Ok((PacketCapturer { cap, sender }, receiver))
    }

    /**
     *  Start the asynchronous packet capturing loop
     */
    pub fn start_capture_loop(mut self) {
        // Clone the sender to move into the thread
        let sender = self.sender.clone();
        // Capture needs to be in a blocking task since pcap::Capture is blocking
        task::spawn_blocking(move || {
            loop {
                match self.cap.next_packet() {
                    Ok(packet) => {
                        let packet = OwnedPacket::from(packet);
                        if sender.send(packet).is_err() {
                            // Receiver has been dropped
                            error!("Receiver dropped. Stopping packet capture.");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Error capturing packet: {}", e);
                        continue;
                    }
                }
            }
        });
    }
}
