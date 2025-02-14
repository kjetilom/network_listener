-- First, enable the TimescaleDB extension (if not already enabled)
CREATE EXTENSION IF NOT EXISTS timescaledb;

CREATE TABLE IF NOT EXISTS link (
    id SERIAL PRIMARY KEY,
    sender_ip TEXT NOT NULL,
    receiver_ip TEXT NOT NULL,
    UNIQUE(sender_ip, receiver_ip)
);

-- Table for timeseries data for each link.
-- Store a Unix timestamp (in seconds) as a BIGINT.
CREATE TABLE IF NOT EXISTS link_state (
    id SERIAL PRIMARY KEY,
    link_id INTEGER NOT NULL REFERENCES link(id) ON DELETE CASCADE,
    thp_in DOUBLE PRECISION,
    thp_out DOUBLE PRECISION,
    bw DOUBLE PRECISION,
    abw DOUBLE PRECISION,
    latency DOUBLE PRECISION,
    delay DOUBLE PRECISION,
    jitter DOUBLE PRECISION,
    loss DOUBLE PRECISION,
    ts TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS rtt (
    id SERIAL PRIMARY KEY,
    link_id INTEGER NOT NULL REFERENCES link(id) ON DELETE CASCADE,
    rtt DOUBLE PRECISION,
    ts TIMESTAMPTZ NOT NULL
);

CREATE INDEX ON link_state (link_id);
CREATE INDEX ON rtt (link_id);

-- Convert the link_state table into a hypertable using ts as the time column.
SELECT create_hypertable('rtt', 'ts');
SELECT create_hypertable('link_state', 'ts');