CREATE OR REPLACE FUNCTION is_pre_filter_candidate (
    p_len       double precision,
    p_gin       double precision,
    p_gout      double precision,
    p_capacity  double precision,
    p_packet_sz double precision
) RETURNS boolean
  LANGUAGE SQL
  STABLE                     -- not strictly IMMUTABLE, since capacity/packet_sz vary
AS $$
    SELECT
      p_len      >= p_packet_sz
      AND p_gin  >  0
      AND p_gout >  0
      AND p_len / p_gin  <= p_capacity
      AND p_len / p_gout <= p_capacity;
$$;

CREATE OR REPLACE FUNCTION is_regression_candidate(
    p_len        double precision,
    p_gin        double precision,
    p_gout       double precision,
    p_threshold  double precision,
    p_capacity   double precision,
    p_packet_sz  double precision
) RETURNS boolean
  LANGUAGE SQL
  STABLE
AS $$
    SELECT
      is_pre_filter_candidate(p_len, p_gin, p_gout, p_capacity, p_packet_sz)
      AND p_gin <= p_threshold;
$$;


CREATE OR REPLACE FUNCTION get_regression_candidates(
    p_capacity    DOUBLE PRECISION,
    p_packet_sz   DOUBLE PRECISION,
    p_experiment  INTEGER,
    p_quantile    DOUBLE PRECISION DEFAULT 0.10
)
RETURNS TABLE (
    len                  DOUBLE PRECISION,
    gin                  DOUBLE PRECISION,
    gout                 DOUBLE PRECISION,
    link_id              INTEGER,
    link_state_id        INTEGER,
    "time"               TIMESTAMP,
    num_acked            INTEGER,
    experiment_id        INTEGER,
    used_in_regression   BOOLEAN
)
LANGUAGE SQL STABLE
AS $$
WITH

  pre_filter AS (
    SELECT
      *,
      is_pre_filter_candidate(
        len, gin, gout,
        p_capacity,
        p_packet_sz
      ) AS to_use
    FROM pgm_detailed
    WHERE experiment_id = p_experiment
  ),

  ranked AS (
    SELECT
      gout,
      ROW_NUMBER() OVER (ORDER BY gout) AS rn,
      COUNT(*)       OVER ()            AS total_count
    FROM pre_filter
    WHERE to_use
  ),

  avg_low_p AS (
    SELECT AVG(gout) AS threshold
    FROM ranked
    WHERE rn <= CEIL(total_count * p_quantile)
  )

SELECT
  pf.len,
  pf.gin,
  pf.gout,
  pf.link_id,
  pf.link_state_id,
  pf.time,
  pf.num_acked,
  pf.experiment_id,
  is_regression_candidate(
    pf.len,
    pf.gin,
    pf.gout,
    al.threshold,
    p_capacity,
    p_packet_sz
  ) AS used_in_regression
FROM pre_filter AS pf
CROSS JOIN avg_low_p AS al
ORDER BY pf.time;
$$;

CREATE OR REPLACE FUNCTION get_regression_counts_by_timestamp(
    p_capacity   DOUBLE PRECISION,
    p_packet_sz  DOUBLE PRECISION,
    p_experiment INTEGER,
    p_quantile   DOUBLE PRECISION DEFAULT 0.10
)
RETURNS TABLE (
    link_id             INTEGER,
    link_state_id       INTEGER,
    experiment_id       INTEGER,
    used_in_regression  BIGINT,
    unused_in_regression BIGINT
)
LANGUAGE SQL STABLE
AS $$
SELECT
  ls.link_id,
  ls.id              AS link_state_id,
  ls.experiment_id,
  -- count how many of the timestamp’s flags are true / false
  COUNT(*) FILTER (WHERE rc.used_in_regression)       AS used_in_regression,
  COUNT(*) FILTER (WHERE NOT rc.used_in_regression)   AS unused_in_regression
FROM
  -- get_regression_by_timestamp returns one row per pgm datapoint,
  -- with link_id, time, experiment_id, used_in_regression flag
  get_regression_by_timestamp(
    p_capacity,
    p_packet_sz,
    p_experiment,
    p_quantile
  ) AS rc
  -- join back to link_states to pull the state’s PK
  JOIN link_state AS ls
    ON rc.link_id       = ls.link_id
   AND rc.time          = ls.time
   AND ls.experiment_id = p_experiment
GROUP BY
  ls.link_id,
  ls.id,
  ls.experiment_id
ORDER BY
  ls.link_id,
  ls.experiment_id;
$$;


CREATE OR REPLACE FUNCTION get_regression_by_timestamp(
    p_capacity    DOUBLE PRECISION,
    p_packet_sz   DOUBLE PRECISION,
    p_experiment  INTEGER,
    p_quantile    DOUBLE PRECISION DEFAULT 0.10
)
RETURNS TABLE (
    len                DOUBLE PRECISION,
    gin                DOUBLE PRECISION,
    gout               DOUBLE PRECISION,
    link_id            INTEGER,
    link_state_id        INTEGER,
    "time"               TIMESTAMP,
    num_acked          INTEGER,
    experiment_id      INTEGER,
    used_in_regression BOOLEAN
)
LANGUAGE SQL STABLE
AS $$
WITH
  -- A) Apply the basic len/gin/gout filters once
  pre_filter AS (
    SELECT
      *,
      is_pre_filter_candidate(
        len, gin, gout,
        p_capacity,
        p_packet_sz
      ) AS to_use
    FROM pgm_detailed
    WHERE experiment_id = p_experiment
  ),

  -- B) Within each timestamp, rank the surviving rows by gout
  ranked AS (
    SELECT
      pf.time,
      pf.gout,
      ROW_NUMBER() OVER (PARTITION BY pf.time ORDER BY pf.gout) AS rn,
      COUNT(*)       OVER (PARTITION BY pf.time) AS total_count
    FROM pre_filter AS pf
    WHERE pf.to_use
  ),

  -- C) Compute each timestamp’s bottom-10% average gout
  avg_low AS (
    SELECT
      time,
      AVG(gout) AS threshold
    FROM ranked
    WHERE rn <= CEIL(total_count * p_quantile)
    GROUP BY time
  )

-- D) Emit every row, joining its timestamp’s threshold
SELECT
  pf.len,
  pf.gin,
  pf.gout,
  pf.link_id,
  pf.link_state_id,
  pf.time,
  pf.num_acked,
  pf.experiment_id,
  -- TRUE only if it passes the pre-filter AND its gin ≤ that time’s threshold
  CASE
    WHEN pf.to_use
      AND pf.gin <= al.threshold
    THEN TRUE
    ELSE FALSE
  END AS used_in_regression
FROM pre_filter AS pf
LEFT JOIN avg_low AS al
  ON pf.time = al.time
ORDER BY pf.time;
$$;

CREATE OR REPLACE FUNCTION subnet(
    p_ip1 TEXT,
    p_ip2 TEXT DEFAULT ''
) RETURNS TEXT
LANGUAGE SQL STABLE
AS $$
    SELECT
        CASE
            WHEN p_ip1 != '' THEN
                SUBSTRING(p_ip1 FROM '([0-9]+\.[0-9]+\.[0-9]+)\.[0-9]+')
            WHEN p_ip2 != '' THEN
                SUBSTRING(p_ip2 FROM '([0-9]+\.[0-9]+\.[0-9]+)\.[0-9]+')
        END;
$$;

CREATE VIEW throughputs_moving_avg AS
WITH with_subnet AS (
    SELECT
        *,
        subnet(ip41, ip42) AS subnet
    FROM
        throughputs_filtered
)
SELECT
    ws.*,
    AVG(ws.throughput)
        OVER (
            PARTITION BY ws.subnet
            ORDER BY ws.time
            RANGE BETWEEN INTERVAL '35 seconds' PRECEDING
            AND CURRENT ROW
        ) AS moving_avg
FROM with_subnet AS ws
ORDER BY ws.subnet, ws.time;



CREATE VIEW non_interpolated_throughputs AS
WITH
  base_throughputs AS (
    SELECT
      tf.time,
      tf.subnet,
      tf.experiment_id,
      tf.throughput,
      tf.moving_avg
    FROM throughputs_moving_avg AS tf
  ),

  -- every (time, subnet, experiment_id) needed from link_states
  link_timestamps AS (
    SELECT DISTINCT
      ls.time                    AS time,
      subnet(ls.sender_ip)       AS subnet,
      ls.experiment_id           AS experiment_id
    FROM link_states AS ls
    UNION
    SELECT DISTINCT
      ls.time,
      subnet(ls.receiver_ip),
      ls.experiment_id
    FROM link_states AS ls
  ),

  -- those slots not already in base_throughputs get NULLs
  missing_slots AS (
    SELECT
      lt.time,
      lt.subnet,
      lt.experiment_id,
      NULL::double precision AS throughput,
      NULL::double precision AS moving_avg
    FROM link_timestamps AS lt
    LEFT JOIN base_throughputs AS bt
      ON bt.time          = lt.time
     AND bt.subnet        = lt.subnet
     AND bt.experiment_id = lt.experiment_id
    WHERE bt.time IS NULL
  )

SELECT *
FROM base_throughputs

UNION ALL

SELECT *
FROM missing_slots

ORDER BY experiment_id, subnet, time;



CREATE VIEW link_states_with_subnet AS
SELECT
    ls.*,
    l.sender_ip,
    l.receiver_ip,
    subnet(l.sender_ip) AS subnet_snd,
    subnet(l.receiver_ip) AS subnet_rcv
FROM
    link_state AS ls
    JOIN link AS l
        ON l.id = ls.link_id;


-- For each link state, get the corresponding maximum throughput


