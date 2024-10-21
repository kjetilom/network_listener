use std::collections::HashMap;
use procfs::net::TcpState;
use super::stream_id::TcpStreamId;
use neli_wifi::AsyncSocket;
use std::error::Error;

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

pub async fn delay() {
    let delay = std::time::Duration::from_secs(2);

    let mut prev_stats = procfs::net::dev_status().unwrap();
    let mut prev_now = std::time::Instant::now();
    loop {
        std::thread::sleep(delay);
        let now = std::time::Instant::now();
        let dev_stats = procfs::net::dev_status().unwrap();
        let wifi_status = procfs::net::dev_status();

        // calculate diffs from previous
        let dt = (now - prev_now).as_millis() as f32 / 1000.0;

        let mut stats: Vec<_> = dev_stats.values().collect();
        stats.sort_by_key(|s| &s.name);
        println!();
        println!(
            "{:>16}: {:<20}               {:<20} ",
            "Interface", "bytes recv", "bytes sent"
        );
        println!(
            "{:>16}  {:<20}               {:<20}",
            "================", "====================", "===================="
        );
        for stat in stats {
            println!(
                "{:>16}: {:<20}  {:>6.1} kbps  {:<20}  {:>6.1} kbps ",
                stat.name,
                stat.recv_bytes,
                (stat.recv_bytes - prev_stats.get(&stat.name).unwrap().recv_bytes) as f32 / dt / 1000.0,
                stat.sent_bytes,
                (stat.sent_bytes - prev_stats.get(&stat.name).unwrap().sent_bytes) as f32 / dt / 1000.0
            );
        }

        prev_stats = dev_stats;
        prev_now = now;
    }
}

///! Pretty print host interfaces.

pub async fn get_socket_info() -> Result<(), Box<dyn Error>> {
    let mut socket = AsyncSocket::connect()?;

    for interface in socket.get_interfaces_info().await? {
        dbg!(&interface);
        println!("Interface: {}", String::from_utf8_lossy(&interface.name.unwrap()));

        if let Some(index) = interface.index {
            dbg!(socket.get_station_info(index).await?);
            dbg!(socket.get_bss_info(index).await?);
        }

        eprintln!("---");
    }

    Ok(())
}