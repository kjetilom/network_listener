use crate::proto_bw::{BandwidthMessage, PgmMessage, Rtts};
use chrono::{DateTime, TimeZone, Utc};
use log::error;
use tokio_postgres::{types::Timestamp, Client};

use super::core_grpc::ThroughputDP;

// alias PostgreSQL TIMESTAMPTZ wrapper for clarity.
type TstampTZ = Timestamp<DateTime<Utc>>;

fn timestamp_to_datetime(timestamp: i64) -> Option<TstampTZ> {
    // Use Utc.timestamp_millis for milliseconds
    let dtime = Utc.timestamp_millis_opt(timestamp).single()?;
    Some(TstampTZ::Value(dtime))
}

pub async fn get_and_insert_experiment(
    client: &Client,
    experiment_name: &str,
    description: &str,
) -> Result<i32, tokio_postgres::Error> {
    let query = r#"
WITH ins AS (
  INSERT INTO experiment (name, description)
  VALUES ($1, $2)
  ON CONFLICT (name) DO NOTHING
  RETURNING id
)
SELECT id FROM ins
UNION ALL
SELECT id FROM experiment WHERE name = $1
LIMIT 1
"#;
    let row = client
        .query_one(query, &[&experiment_name, &description])
        .await?;
    Ok(row.get(0))
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
    let timeseries_placeholders: Vec<String> =
        (3..(3 + num_vals)).map(|i| format!("${}", i)).collect();
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

    // Builds parameter list: first two parameters are sender_ip and receiver_ip,
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

pub async fn upload_probe_gap_measurements(msg: PgmMessage, client: &Client, experiment_id: i32) {
    // For RTT data, our table (named "rtt") has columns: rtt and ts.
    let cols = ["time", "gin", "gout", "len", "num_acked", "experiment_id"];

    for pgmmsg in &msg.pgm_dps {
        // Convert timestamp to a DateTime<Utc>
        let ts = match timestamp_to_datetime(pgmmsg.timestamp) {
            Some(ts) => ts,
            None => {
                eprintln!("Error converting timestamp to DateTime<Utc> for PGM");
                continue;
            }
        };

        for pgm_dp in pgmmsg.pgm_dp.iter() {
            let values: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = vec![
                &ts,
                &pgm_dp.gin,
                &pgm_dp.gout,
                &pgm_dp.len,
                &pgm_dp.num_acked,
                &experiment_id,
            ];
            insert_into(
                client,
                &pgmmsg.sender_ip,
                &pgmmsg.receiver_ip,
                "pgm",
                &cols,
                &values,
            )
            .await;
        }
    }
}

pub async fn upload_throughput(msg: Vec<ThroughputDP>, client: &Client, experiment_id: i32) {
    let cols = [
        "node1",
        "iface1",
        "ip41",
        "node2",
        "iface2",
        "ip42",
        "throughput",
        "time",
        "experiment_id",
    ];

    for thput in msg {
        // Convert timestamp (milliseconds) to a DateTime<Utc>
        let ts = match timestamp_to_datetime(thput.timestamp as i64) {
            Some(ts) => ts,
            None => {
                eprintln!("Error converting timestamp to DateTime<Utc> for throughput");
                continue;
            }
        };

        let values: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = vec![
            &thput.node1,
            &thput.iface1,
            &thput.ip41,
            &thput.node2,
            &thput.iface2,
            &thput.ip42,
            &thput.throughput,
            &ts,
            &experiment_id,
        ];
        let query = format!(
            "INSERT INTO throughput ({}) VALUES ({})",
            cols.join(", "),
            (1..=values.len())
                .map(|i| format!("${}", i))
                .collect::<Vec<_>>()
                .join(", ")
        );

        if let Err(e) = client.execute(&query, &values).await {
            eprintln!("Error inserting record: {}", e);
        }
    }
}

/// Uploads bandwidth data (for each LinkState) into the database.
pub async fn upload_bandwidth(msg: BandwidthMessage, client: &Client, experiment_id: i32) {
    let cols = [
        "thp_in",
        "thp_out",
        "bw",
        "abw",
        "latency",
        "delay",
        "jitter",
        "loss",
        "time",
        "experiment_id",
    ];

    for ls in &msg.link_state {
        // Convert timestamp (milliseconds) to a DateTime<Utc>
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
            &experiment_id,
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

/// Uploads RTT data (for each Rtt) into the database.
pub async fn upload_rtt(msg: Rtts, client: &Client, experiment_id: i32) {
    // For RTT data, our table (named "rtt") has columns: rtt and ts.
    let cols = ["rtt", "time", "experiment_id"];

    for rttmsg in &msg.rtts {
        for rtt in &rttmsg.rtt {
            let ts = match timestamp_to_datetime(rtt.timestamp) {
                Some(ts) => ts,
                None => {
                    error!("Error converting timestamp to DateTime<Utc> for RTT");
                    continue;
                }
            };

            let values: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
                vec![&rtt.rtt, &ts, &experiment_id];

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
