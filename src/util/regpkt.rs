
// Used to store packets which are acked, or sent (udp) or received (tcp) packets.

use std::{collections::VecDeque, ops::{Deref, DerefMut}};

#[derive(Debug)]
pub struct PacketRegistry {
    packets: VecDeque<RegPkt>,
    some_other_field: u32, // ! FIXME
    // ...
}

impl PacketRegistry {
    pub fn new(size: usize) -> Self {
        PacketRegistry {
            packets: VecDeque::with_capacity(size),
            some_other_field: 0,
        }
    }

    pub fn push(&mut self, value: RegPkt) -> RegPkt {
        if self.packets.len() == self.packets.capacity() {
            let old = self.packets.pop_front().unwrap();
            // Do something with old
        }
        self.packets.push_back(value);

        self.packets.back().unwrap().clone()
    }
}

impl Deref for PacketRegistry {
    type Target = VecDeque<RegPkt>;

    fn deref(&self) -> &Self::Target {
        &self.packets
    }
}

impl DerefMut for PacketRegistry {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.packets
    }
}

/// Single struct to represent a sent or received packet.
/// Should be as small as possible to reduce memory usage.
///
/// # Fields
///
/// * `payload_len` - Length of the packet payload.
/// * `total_length` - Total length of the packet.
/// * `sent_time` - Time when the packet was sent.
/// * `retransmissions` - Number of retransmissions for the packet.
/// * `rtt` - Round trip time to acknowledge the segment.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RegPkt {
    pub payload_len: u16,
    pub total_length: u16,
    pub sent_time: std::time::SystemTime, // TODO: Change to relative time
    pub retransmissions: u8,
    pub rtt: Option<std::time::Duration>, // TODO: Change to u32 micros duration is like 20 bytes or something
}
