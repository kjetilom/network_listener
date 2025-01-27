use std::collections::HashMap;
// use super::stream_id::StreamId;
use neli_wifi::{AsyncSocket, Interface};
use pnet::packet::ip::IpNextHeaderProtocols;
use std::error::Error;
use super::{parser::NetlinkData, tracker::stream_id::ConnectionKey};
use procfs::net::{TcpNetEntry, UdpNetEntry};

pub enum NetEntry {
    Tcp{
        entry: TcpNetEntry
    },
    Udp{
        entry: UdpNetEntry
    },
}

pub struct NetStat {
    pub tcp: HashMap<ConnectionKey, NetEntry>,
    pub udp: HashMap<ConnectionKey, NetEntry>,
}


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
            ConnectionKey::from_tcp_net_entry(&tcp_entry, IpNextHeaderProtocols::Tcp),
            NetEntry::Tcp {
                entry: tcp_entry
            }
        );
    }
    for udp_entry in udp_entries {
        nstat.udp.insert(
            ConnectionKey::from_udp_net_entry(&udp_entry,IpNextHeaderProtocols::Udp),
            NetEntry::Udp {
                entry: udp_entry
            }
        );
    }

    nstat
}

pub async fn get_interface_info(index: i32) -> Result<NetlinkData,  Box<dyn std::error::Error + Send + Sync>> {
    let mut socket = AsyncSocket::connect()?;
    let station_info = socket.get_station_info(index).await?;
    let bss_info = socket.get_bss_info(index).await?;
    let neli_data = NetlinkData {
        stations: station_info,
        bss: bss_info,
    };
    return Ok(neli_data);
}

pub async fn get_interface(device_name: &str) -> Result<Interface, Box<dyn Error>> {
    let mut socket = AsyncSocket::connect()?;
    let interfaces = socket.get_interfaces_info().await?;

    for interface in interfaces {
        let interface_name = interface.name.as_ref().unwrap();
        if interface_name.last() == Some(&0) {
            if interface_name[..interface_name.len() - 1] == *device_name.as_bytes() {
                return Ok(interface);
            }
        } else if interface_name == device_name.as_bytes() {
            return Ok(interface);
        }
        // Compare names, take null-terminated string into account

    }
    Err("Interface not found".into())
}
