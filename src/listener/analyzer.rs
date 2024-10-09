use std::time::{Duration, Instant};
use log::info;
use crate::listener::parser::ParsedPacket;

pub struct Analyzer {
    start_time: Instant,
    packet_count: usize,
    byte_count: usize,
}

impl Analyzer {
    pub fn new() -> Self {
        Analyzer {
            start_time: Instant::now(),
            packet_count: 0,
            byte_count: 0,
        }
    }

    pub fn process_packet(&mut self, packet: &ParsedPacket) {
        self.packet_count += 1;
        self.byte_count += packet.payload.len();

        if self.start_time.elapsed() >= Duration::from_secs(1) {
            info!(
                "Packets: {} | mbps: {} | Time elapsed: {}s",
                self.packet_count,
                self.byte_count as f32 * 8.0 / 1_000_000.0,
                self.start_time.elapsed().as_secs()
            );

            self.start_time = Instant::now();
            self.packet_count = 0;
            self.byte_count = 0;
        }
    }
}