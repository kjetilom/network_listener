use std::collections::HashMap;
use crate::stream_id::IpPair;

// use super::stream_id::StreamId;
use super::{
    parser::NetlinkData,
    tracking::stream_id::{from_tcp_net_entry, from_udp_net_entry, StreamKey},
};
use neli_wifi::{AsyncSocket, Interface};
use pnet::packet::ip::IpNextHeaderProtocols;
use procfs::net::{TcpNetEntry, UdpNetEntry};
use std::error::Error;

/// Represents a single network connection entry, either TCP or UDP,
/// as read from `/proc/net/tcp` or `/proc/net/udp` tables.
pub enum NetEntry {
    Tcp { entry: TcpNetEntry },
    Udp { entry: UdpNetEntry },
}

/// Aggregates current TCP and UDP connection states.
///
/// `tcp` and `udp` maps use a key of `(StreamKey, IpPair)` to uniquely
/// identify each connection and wrap the corresponding procfs entry.
#[derive(Default)]
pub struct NetStat {
    pub tcp: HashMap<(StreamKey, IpPair), NetEntry>,
    pub udp: HashMap<(StreamKey, IpPair), NetEntry>,
}

/// Asynchronously reads and parses network connection tables from procfs.
///
/// This function gathers entries from both IPv4 and IPv6 tables for TCP and UDP:
/// - `/proc/net/tcp` and `/proc/net/tcp6`
/// - `/proc/net/udp` and `/proc/net/udp6`
///
/// Each raw entry is converted into a `NetEntry` and inserted into a `NetStat`.
///
/// # Returns
/// A `NetStat` containing the current snapshot of TCP and UDP connections.
pub async fn proc_net() -> NetStat {
    let tcp = [procfs::net::tcp(), procfs::net::tcp6()];
    let udp = [procfs::net::udp(), procfs::net::udp6()];

    let entries = tcp.into_iter().filter_map(|res| res.ok()).flatten();
    let udp_entries = udp.into_iter().filter_map(|res| res.ok()).flatten();

    let mut nstat = NetStat {
        tcp: HashMap::new(),
        udp: HashMap::new(),
    };

    for tcp_entry in entries {
        nstat.tcp.insert(
            from_tcp_net_entry(&tcp_entry, IpNextHeaderProtocols::Tcp),
            NetEntry::Tcp { entry: tcp_entry },
        );
    }
    for udp_entry in udp_entries {
        nstat.udp.insert(
            from_udp_net_entry(&udp_entry, IpNextHeaderProtocols::Udp),
            NetEntry::Udp { entry: udp_entry },
        );
    }
    nstat
}


/// Retrieves wireless interface statistics via Netlink.
///
/// Connects to the kernel using an asynchronous netlink socket,
/// then fetches station (client) and BSS (AP) information for the
/// interface identified by `index`.
pub async fn get_interface_info(
    index: i32,
) -> Result<NetlinkData, Box<dyn std::error::Error + Send + Sync>> {
    let mut socket = AsyncSocket::connect()?;
    let station_info = socket.get_station_info(index).await?;
    let bss_info = socket.get_bss_info(index).await?;
    let neli_data = NetlinkData {
        stations: station_info,
        bss: bss_info,
    };
    Ok(neli_data)
}

/// Finds and returns a wireless `Interface` by name using Netlink.
///
/// Connects to the kernel netlink socket and lists all interfaces;
/// compares each interface’s null-terminated name to `device_name`.
pub async fn get_interface(device_name: &str) -> Result<Interface, Box<dyn Error>> {
    let mut socket = AsyncSocket::connect()?;
    let interfaces = socket.get_interfaces_info().await?;

    for interface in interfaces {
        let interface_name = match interface.name.as_ref() {
            Some(name) => name,
            None => continue,
        };
        if interface_name.last() == Some(&0) {
            if interface_name[..interface_name.len() - 1] == *device_name.as_bytes() {
                return Ok(interface);
            }
        } else if interface_name == device_name.as_bytes() {
            return Ok(interface);
        }
    }
    Err("Interface not found".into())
}
