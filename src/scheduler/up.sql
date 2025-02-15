-- First, enable the TimescaleDB extension (if not already enabled)
CREATE EXTENSION IF NOT EXISTS timescaledb;

CREATE TABLE IF NOT EXISTS link (
    id SERIAL PRIMARY KEY,
    sender_ip TEXT NOT NULL,
    receiver_ip TEXT NOT NULL,
    UNIQUE(sender_ip, receiver_ip)
);

-- Table for timeseries data for each link.
CREATE TABLE IF NOT EXISTS link_state (
    time TIMESTAMPTZ NOT NULL,
    id SERIAL,
    link_id INTEGER NOT NULL REFERENCES link(id) ON DELETE CASCADE,
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

CREATE TABLE IF NOT EXISTS rtt (
    time TIMESTAMPTZ NOT NULL,
    id SERIAL,
    link_id INTEGER NOT NULL REFERENCES link(id) ON DELETE CASCADE,
    rtt DOUBLE PRECISION,
    PRIMARY KEY (time, id)
);

CREATE INDEX ON link_state (link_id);
CREATE INDEX ON rtt (link_id);

-- Convert the tables into hypertables using "time" as the time column.
SELECT create_hypertable('link_state', 'time');
SELECT create_hypertable('rtt', 'time');