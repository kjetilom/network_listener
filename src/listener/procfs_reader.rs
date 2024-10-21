use std::collections::HashMap;
use procfs::net::TcpState;
use super::stream_id::TcpStreamId;

pub fn netstat_test() -> HashMap<TcpStreamId, (TcpState, u32, u32, u64)> {
    // get the tcp table
    let tcp = [procfs::net::tcp(), procfs::net::tcp6()];

    let entries = tcp.into_iter().filter_map(|res| res.ok()).flatten();
    // Iterate over the entries and map to TcpStreamId : State, rx_queue, tx_queue, u3inode,

    let mut stream_map: HashMap<TcpStreamId, (TcpState, u32, u32, u64)> = HashMap::new();

    entries.into_iter().for_each(|entry| {
        stream_map.insert(
            TcpStreamId::from(&entry),
            (entry.state, entry.rx_queue, entry.tx_queue, entry.inode
        ));
    });
    stream_map
}