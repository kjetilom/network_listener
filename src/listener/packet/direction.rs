use pnet::datalink::MacAddr;

#[derive(Debug, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use pnet::datalink::MacAddr;

    #[test]
    fn test_direction_from_mac() {
        let own_mac = MacAddr::new(0, 0, 0, 0, 0, 0);
        let incoming_mac = MacAddr::new(1, 1, 1, 1, 1, 1);
        let outgoing_mac = MacAddr::new(2, 2, 2, 2, 2, 2);

        assert_eq!(Direction::from_mac(own_mac, own_mac), Direction::Incoming);
        assert_eq!(
            Direction::from_mac(incoming_mac, own_mac),
            Direction::Outgoing
        );
        assert_eq!(
            Direction::from_mac(outgoing_mac, own_mac),
            Direction::Outgoing
        );
    }
}
