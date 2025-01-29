use serde::Deserialize;
use serde::Serialize;


#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IperfResponse {
    Success(Success),
    Error(Error),
}
/// ---------------///
/// ERROR response ///
/// ---------------///
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub error: String,
}


/// -----------------///
/// SUCCESS response ///
/// -----------------///
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Success {
    pub intervals: Vec<Interval>,
    pub end: End,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connected {
    pub socket: i64,
    #[serde(rename = "local_host")]
    pub local_host: String,
    #[serde(rename = "local_port")]
    pub local_port: i64,
    #[serde(rename = "remote_host")]
    pub remote_host: String,
    #[serde(rename = "remote_port")]
    pub remote_port: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectingTo {
    pub host: String,
    pub port: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interval {
    pub sum: Sum,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sum {
    pub start: f64,
    pub end: f64,
    pub seconds: f64,
    pub bytes: f64,
    #[serde(rename = "bits_per_second")]
    pub bits_per_second: f64,
    pub retransmits: Option<f64>,
    pub omitted: bool,
    pub sender: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct End {
    pub streams: Vec<Stream2>,
    #[serde(rename = "sum_sent")]
    pub sum_sent: SumSent,
    #[serde(rename = "sum_received")]
    pub sum_received: SumReceived,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stream2 {
    pub sender: Sender,
    pub receiver: Receiver,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sender {
    pub socket: i64,
    pub start: i64,
    pub end: f64,
    pub seconds: f64,
    pub bytes: i64,
    #[serde(rename = "bits_per_second")]
    pub bits_per_second: f64,
    pub retransmits: Option<i64>,
    #[serde(rename = "max_snd_cwnd")]
    pub max_snd_cwnd: Option<i64>,
    #[serde(rename = "max_snd_wnd")]
    pub max_snd_wnd: Option<i64>,
    #[serde(rename = "max_rtt")]
    pub max_rtt: Option<i64>,
    #[serde(rename = "min_rtt")]
    pub min_rtt: Option<i64>,
    #[serde(rename = "mean_rtt")]
    pub mean_rtt: Option<i64>,
    pub sender: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receiver {
    pub socket: i64,
    pub start: i64,
    pub end: f64,
    pub seconds: f64,
    pub bytes: i64,
    #[serde(rename = "bits_per_second")]
    pub bits_per_second: f64,
    pub sender: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SumSent {
    pub start: i64,
    pub end: f64,
    pub seconds: f64,
    pub bytes: i64,
    #[serde(rename = "bits_per_second")]
    pub bits_per_second: f64,
    pub retransmits: Option<i64>,
    pub sender: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SumReceived {
    pub start: i64,
    pub end: f64,
    pub seconds: f64,
    pub bytes: i64,
    #[serde(rename = "bits_per_second")]
    pub bits_per_second: f64,
    pub sender: bool,
}
