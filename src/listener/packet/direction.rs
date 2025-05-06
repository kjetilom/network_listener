use pnet::datalink::MacAddr;


/// Represents the direction of a network packet relative to the local host.
/// The packet is outgoing if it is sent or redirected from the local host.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Direction {
    /// Packet is received by the local host.
    Incoming,
    /// Packet is sent from the local host.
    Outgoing,
}

impl Direction {
    /// Determines packet direction based on a MAC address comparison.
    ///
    /// If the provided `mac` equals the host's `own_mac`, the packet
    /// is treated as incoming; otherwise, outgoing.
    pub fn from_mac(mac: MacAddr, own_mac: MacAddr) -> Self {
        if mac == own_mac {
            Direction::Incoming
        } else {
            Direction::Outgoing
        }
    }

    /// Returns true if this direction is `Incoming`.
    pub fn is_incoming(&self) -> bool {
        matches!(self, Direction::Incoming)
    }

    /// Returns true if this direction is `Outgoing`.
    pub fn is_outgoing(&self) -> bool {
        matches!(self, Direction::Outgoing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pnet::datalink::MacAddr;

    /// Test `from_mac` yields correct `Direction` variants.
    #[test]
    fn test_direction_from_mac() {
        let own_mac = MacAddr::new(0, 0, 0, 0, 0, 0);
        // Same address -> incoming
        assert_eq!(Direction::from_mac(own_mac, own_mac), Direction::Incoming);
        // Different addresses -> outgoing
        let other_mac = MacAddr::new(1, 1, 1, 1, 1, 1);
        assert_eq!(Direction::from_mac(other_mac, own_mac), Direction::Outgoing);
    }

    /// Test `is_incoming` and `is_outgoing` convenience methods.
    #[test]
    fn test_direction_flags() {
        let incoming = Direction::Incoming;
        let outgoing = Direction::Outgoing;

        assert!(incoming.is_incoming());
        assert!(!incoming.is_outgoing());

        assert!(outgoing.is_outgoing());
        assert!(!outgoing.is_incoming());
    }
}
