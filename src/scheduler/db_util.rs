use crate::proto_bw::{BandwidthMessage, Pgm, Rtts};
use chrono::{TimeZone, Utc, DateTime};
use log::error;
use tokio_postgres::{types::Timestamp, Client};

// alias PostgreSQL TIMESTAMPTZ wrapper for clarity.
type TstampTZ = Timestamp<DateTime<Utc>>;

fn timestamp_to_datetime(timestamp: i64) -> Option<TstampTZ> {
    // Use Utc.timestamp_millis for milliseconds
    let dtime = Utc.timestamp_millis_opt(timestamp).single()?;
    Some(TstampTZ::Value(dtime))
}

/// Inserts data into the given table by first upserting the link and then inserting
/// the timeseries data with the proper link_id.
///
/// This function constructs a query like:
///
/// WITH ins AS (
///     INSERT INTO link(sender_ip, receiver_ip)
///     VALUES ($1, $2)
///     ON CONFLICT (sender_ip, receiver_ip) DO NOTHING
///     RETURNING id
/// ),
/// sel AS (
///     SELECT id FROM ins
///     UNION
///     SELECT id FROM link
///     WHERE sender_ip = $1 AND receiver_ip = $2
/// )
/// INSERT INTO {table} (link_id, {col1}, {col2}, ..., {colN})
/// VALUES ((SELECT id FROM sel), $3, $4, ..., ${2+N})
///
pub async fn insert_into(
    client: &Client,
    sender_ip: &str,
    receiver_ip: &str,
    table: &str,
    columns: &[&str],
    values: &[&(dyn tokio_postgres::types::ToSql + Sync)],
) {
    // Build placeholders for timeseries values: they start at parameter $3.
    let num_vals = values.len();
    let timeseries_placeholders: Vec<String> = (3..(3 + num_vals))
        .map(|i| format!("${}", i))
        .collect();
    let timeseries_placeholders_str = timeseries_placeholders.join(", ");
    let columns_str = columns.join(", ");

    let query = format!(
        "WITH ins AS (
            INSERT INTO link(sender_ip, receiver_ip)
            VALUES ($1, $2)
            ON CONFLICT (sender_ip, receiver_ip) DO NOTHING
            RETURNING id
        ),
        sel AS (
            SELECT id FROM ins
            UNION
            SELECT id FROM link
            WHERE sender_ip = $1 AND receiver_ip = $2
        )
        INSERT INTO {} (link_id, {}) VALUES ((SELECT id FROM sel), {})",
        table, columns_str, timeseries_placeholders_str
    );

    // Build parameter list: first two parameters are sender_ip and receiver_ip,
    // then the values for the timeseries columns.
    let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = Vec::new();
    params.push(&sender_ip);
    params.push(&receiver_ip);
    for v in values {
        params.push(*v);
    }

    if let Err(e) = client.execute(&query, &params).await {
        eprintln!("Error inserting record: {}", e);
    }
}

pub async fn upload_probe_gap_measurements(msg: Pgm, _client: &Client) {
    println!("{:?}", msg);
}

/// Uploads bandwidth data (for each LinkState) into the database.
pub async fn upload_bandwidth(msg: BandwidthMessage, client: &Client) {
    let cols = [
        "thp_in", "thp_out", "bw", "abw", "latency", "delay", "jitter", "loss", "time",
    ];

    for ls in &msg.link_state {
        // Convert timestamp (assuming milliseconds) to a DateTime<Utc>
        let ts = match timestamp_to_datetime(ls.timestamp) {
            Some(ts) => ts,
            None => {
                eprintln!("Error converting timestamp to DateTime<Utc> for bandwidth");
                continue;
            }
        };

        let values: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = vec![
            &ls.thp_in,
            &ls.thp_out,
            &ls.bw,
            &ls.abw,
            &ls.latency,
            &ls.delay,
            &ls.jitter,
            &ls.loss,
            &ts,
        ];

        insert_into(
            client,
            &ls.sender_ip,
            &ls.receiver_ip,
            "link_state",
            &cols,
            &values,
        )
        .await;
    }
}

/// Uploads RTT data into the database.
///
/// This assumes that each Rtts message contains a sender and receiver IP as well.
/// (Adjust the structure if needed.)
pub async fn upload_rtt(msg: Rtts, client: &Client) {
    // For RTT data, our table (named "rtt") has columns: rtt and ts.
    let cols = ["rtt", "time"];

    for rttmsg in &msg.rtts {
        // Assuming rttmsg has sender_ip and receiver_ip fields.
        // If not, adjust accordingly.
        for rtt in &rttmsg.rtt {
            let ts = match timestamp_to_datetime(rtt.timestamp) {
                Some(ts) => ts,
                None => {
                    error!("Error converting timestamp to DateTime<Utc> for RTT");
                    continue;
                }
            };

            let values: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
                vec![&rtt.rtt, &ts];

            // Adjust these fields according to your actual Rtts message structure.
            insert_into(
                client,
                &rttmsg.sender_ip,
                &rttmsg.receiver_ip,
                "rtt",
                &cols,
                &values,
            )
            .await;
        }
    }
}