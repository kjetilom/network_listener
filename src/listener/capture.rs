use pcap::{Capture, Device, Packet, Active};
use std::error::Error;
use log::{error, info};

pub struct PacketCapturer {
    cap: Capture<Active>,
}

impl PacketCapturer {

    /*
     * Create a new PacketCapturer instance
     */
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let device = Device::lookup()?
            .ok_or("No device available for capture")?;
        info!("Using device: {}", device.name);

        let cap = Capture::from_device(device)?
            .promisc(true)
            .immediate_mode(true)
            .timeout(0)
            .open()?;

        Ok(PacketCapturer { cap })
    }

    /*
     * Capture packets in a loop
     */
    pub fn capture_loop<F>(&mut self, mut packet_handler: F) ->
        Result<(), Box<dyn Error>> where
            F: FnMut(Packet),
    {
        loop {
            match self.cap.next_packet() {
                Ok(packet) => packet_handler(packet),
                Err(e) => {
                    error!("Error capturing packet: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }
}