use pnet::datalink::MacAddr;

#[derive(Debug)]
pub enum Direction {
    Incoming,
    Outgoing,
}

impl Direction {
    pub fn from_mac(mac: MacAddr, own_mac: MacAddr) -> Self {
        if mac == own_mac {
            Direction::Incoming
        } else {
            Direction::Outgoing
        }
    }

    pub fn is_incoming(&self) -> bool {
        matches!(self, Direction::Incoming)
    }

    pub fn is_outgoing(&self) -> bool {
        matches!(self, Direction::Outgoing)
    }
}
