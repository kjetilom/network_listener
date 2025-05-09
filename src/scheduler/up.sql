-- These tables and views are used to store experiment data for later analysis.

CREATE EXTENSION IF NOT EXISTS timescaledb;

CREATE TABLE
    IF NOT EXISTS experiment (
        id SERIAL PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT NOT NULL,
        UNIQUE (name)
    );

CREATE TABLE
    IF NOT EXISTS experiment_config(
        experiment_id INTEGER NOT NULL REFERENCES experiment (id) ON DELETE CASCADE,
    );

CREATE TABLE
    IF NOT EXISTS link (
        id SERIAL PRIMARY KEY,
        sender_ip TEXT NOT NULL,
        receiver_ip TEXT NOT NULL,
        UNIQUE (sender_ip, receiver_ip)
    );

-- Table for timeseries data for each link.
CREATE TABLE
    IF NOT EXISTS link_state (
        time TIMESTAMPTZ NOT NULL,
        id SERIAL,
        link_id INTEGER NOT NULL REFERENCES link (id) ON DELETE CASCADE,
        experiment_id INTEGER NOT NULL REFERENCES experiment (id) ON DELETE CASCADE,
        thp_in DOUBLE PRECISION,
        thp_out DOUBLE PRECISION,
        bw DOUBLE PRECISION,
        abw DOUBLE PRECISION,
        latency DOUBLE PRECISION,
        delay DOUBLE PRECISION,
        jitter DOUBLE PRECISION,
        loss DOUBLE PRECISION,
        PRIMARY KEY (time, id)
    );

CREATE TABLE
    IF NOT EXISTS pgm (
        time TIMESTAMPTZ NOT NULL,
        id SERIAL,
        link_id INTEGER NOT NULL REFERENCES link (id) ON DELETE CASCADE,
        experiment_id INTEGER NOT NULL REFERENCES experiment (id) ON DELETE CASCADE,
        gin DOUBLE PRECISION,
        gout DOUBLE PRECISION,
        len INTEGER,
        num_acked INTEGER,
        PRIMARY KEY (id, link_id)
    );

CREATE TABLE
    IF NOT EXISTS rtt (
        time TIMESTAMPTZ NOT NULL,
        id SERIAL,
        link_id INTEGER NOT NULL REFERENCES link (id) ON DELETE CASCADE,
        rtt DOUBLE PRECISION,
        PRIMARY KEY (time, id)
    );

CREATE TABLE
    IF NOT EXISTS throughput (
        time TIMESTAMPTZ NOT NULL,
        id SERIAL,
        experiment_id INTEGER NOT NULL REFERENCES experiment (id) ON DELETE CASCADE,
        node1 TEXT NOT NULL,
        iface1 TEXT NOT NULL,
        ip41 TEXT NOT NULL,
        node2 TEXT NOT NULL,
        iface2 TEXT NOT NULL,
        ip42 TEXT NOT NULL,
        throughput DOUBLE PRECISION
    );


CREATE VIEW
    throughputs_filtered AS
SELECT DISTINCT ON (throughput, time)
    throughput.*,
    exp.name AS experiment_name
FROM
    throughput
    JOIN experiment exp ON throughput.experiment_id = exp.id
WHERE
    throughput.node1 = 'n2' OR throughput.node2 = 'n2'
ORDER BY time ASC;

CREATE VIEW
    pgm_detailed AS
SELECT
    l.sender_ip as sender_ip,
    l.receiver_ip as receiver_ip,
    ls.id AS link_state_id,
    exp.name AS experiment_name,
    pgm.*
FROM
    pgm
    JOIN link l ON pgm.link_id = l.id
    JOIN experiment exp ON pgm.experiment_id = exp.id
    JOIN link_state ls ON pgm.link_id = ls.link_id AND pgm.time = ls.time
ORDER BY
    pgm.time ASC;



CREATE VIEW
    pgm_dps AS
SELECT
    l.sender_ip as sender_ip,
    l.receiver_ip as receiver_ip,
    pgm.gin as gin,
    pgm.gout as gout,
    pgm.len as len,
    pgm.num_acked as num_acked,
    pgm.time as time
FROM
    pgm pgm
    JOIN link l ON pgm.link_id = l.id;

CREATE VIEW
    link_states AS
SELECT
    l.sender_ip as sender_ip,
    l.receiver_ip as receiver_ip,
    ls.thp_in as thp_in,
    ls.thp_out as thp_out,
    ls.bw as bw,
    ls.abw as abw,
    ls.latency as latency,
    ls.delay as delay,
    ls.jitter as jitter,
    ls.loss as loss,
    ls.experiment_id as experiment_id,
    ls.time as time
FROM
    link_state ls
    JOIN link l ON ls.link_id = l.id;

CREATE VIEW
    latency AS
SELECT
    time_bucket ('10 second', ls.time) AS time,
    AVG(ls.latency) AS value,
    CONCAT (l.sender_ip, ' -> ', l.receiver_ip) AS metric
FROM
    link_state ls
    JOIN link l ON ls.link_id = l.id
GROUP BY
    time,
    metric
ORDER BY
    metric,
    time ASC;

CREATE VIEW
    links AS
SELECT
    link.id AS id1,
    l.id AS id2,
    link.sender_ip,
    link.receiver_ip
FROM
    link
    JOIN link l ON link.sender_ip = l.receiver_ip
    AND link.receiver_ip = l.sender_ip
WHERE
    link.id < l.id;

CREATE INDEX ON link_state (link_id);

CREATE INDEX ON rtt (link_id);

CREATE INDEX ON pgm (link_id);

CREATE INDEX ON pgm (experiment_id);

CREATE INDEX ON link_state (experiment_id);

CREATE INDEX ON throughput (experiment_id);

SELECT
    create_hypertable ('link_state', 'time');

SELECT
    create_hypertable ('rtt', 'time');

SELECT
    create_hypertable ('pgm', 'link_id');
