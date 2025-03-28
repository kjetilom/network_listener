-- First, enable the TimescaleDB extension (if not already enabled)
CREATE EXTENSION IF NOT EXISTS timescaledb;

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
        gin DOUBLE PRECISION,
        gout DOUBLE PRECISION,
        len DOUBLE PRECISION,
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

-- Create a view to calculate the average latency for each link.
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

-- Links that are bidirectional.
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

-- Convert the tables into hypertables using "time" as the time column.
SELECT
    create_hypertable ('link_state', 'time');

SELECT
    create_hypertable ('rtt', 'time');
