use std::{collections::HashMap, time::{Duration, SystemTime, UNIX_EPOCH}};
use crate::listener::stream_id::ConnectionKey;

/*
 * The link module is responsible for gathering statistics about the network traffic.
 * The data is first gathered by the trackers, such as tcp_tracker, which gathers RTT measurements.
 * The link manager is then responsible for storing the information for later use.
 *
 */



type Links = HashMap<ConnectionKey, Link>;
type Link = Vec<DataPoint>;

pub trait Timeseries<T> {
    fn extend(&mut self, data: T);
    fn get_datapoints(&self, start: u64, end: u64) -> Vec<&T>;
    fn flush(&mut self) -> Vec<T>;
    fn to_string(&self) -> String;
}

#[derive(Debug, Clone)]
pub struct LinkManager {
    pub links: Links,
}

#[derive(Debug, Clone)]
pub struct DataPoint {
    pub total_size: u16,
    pub timestamp: SystemTime,
    pub rtt_usec: Option<u32>,
    // Add more later
}

impl DataPoint {
    pub fn new(total_size: u16, timestamp: SystemTime, rtt_usec: Option<u32>) -> Self {
        DataPoint {
            total_size,
            timestamp,
            rtt_usec,
        }
    }

    pub fn to_string(&self, start_time: SystemTime) -> String {
        // Return only the existing fields
        let total_size = self.total_size.to_string();
        let timestamp = self.timestamp.duration_since(start_time).unwrap_or(Duration::new(0, 0)).as_micros().to_string();
        let rtt_usec = match self.rtt_usec {
            Some(rtt) => rtt.to_string(),
            None => "".to_string(),
        };

        format!("{},{},{}", total_size, timestamp, rtt_usec)
    }
}

impl LinkManager {
    pub fn new() -> Self {
        LinkManager {
            links: HashMap::new(),
        }
    }

    pub fn record_data(&mut self, key: ConnectionKey, data: Link) {
        let ck = key.as_generic();

        self.links.entry(ck)
            .or_insert(Link::new())
            .extend(data);
    }

    pub fn add_data_points(&mut self, key: ConnectionKey, data: Vec<DataPoint>) {
        let ck = key.as_generic();

        self.links.entry(ck)
            .or_insert(Link::new())
            .extend(data);
    }

    pub fn dump_to_file(&self, start_time: SystemTime) {
        // Create file if it doesnt exist, otherwise append
        // Filename = "link_data.txt"
        let mut lines = Vec::new();
        for (key, link) in self.links.iter() {
            let hdr = key.to_string();
            lines.push(hdr);
            for dp in link {
                lines.push(dp.to_string(start_time));
            }
        }
        // Write to file
        std::fs::write("link_data.txt", lines.join("\n")).expect("Unable to write file");
    }
}

