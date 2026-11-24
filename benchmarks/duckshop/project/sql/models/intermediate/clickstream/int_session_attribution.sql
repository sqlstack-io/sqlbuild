MODEL (
  description """
    One row per web session with UTM parameters parsed from the landing URL, a reporting channel
    from the channel mapping seed, and the visitor's first-touch and last-touch channels.
  """,
  columns (
    session_id (type VARCHAR, nullable false, audits [not_null, unique]),
    anonymous_id (type VARCHAR),
    customer_id (type VARCHAR),
    session_start_utc (type TIMESTAMP_NTZ),
    utm_source (type VARCHAR),
    utm_medium (type VARCHAR),
    utm_campaign (type VARCHAR),
    channel_code (type VARCHAR, audits [not_null]),
    is_paid (type BOOLEAN),
    first_touch_channel (type VARCHAR),
    last_touch_channel (type VARCHAR),
    touch_number (type INT),
  ),
);

WITH sessions AS (
  SELECT *
  FROM __ref("int_web_sessions")
),

channel_mapping AS (
  SELECT *
  FROM __seed("channel_mapping")
),

parsed AS (
  SELECT
    s.session_id,
    s.anonymous_id,
    s.customer_id,
    s.session_start_utc,
    s.referrer_host,
    LOWER(PARSE_URL(s.landing_page_url, 1)['parameters']['utm_source']::VARCHAR) AS utm_source,
    LOWER(
      COALESCE(
        PARSE_URL(s.landing_page_url, 1)['parameters']['utm_medium']::VARCHAR,
        REGEXP_SUBSTR(s.landing_page_url, '[?&]utm_medium=([^&#]+)', 1, 1, 'e', 1)
      )
    ) AS utm_medium,
    LOWER(
      REGEXP_SUBSTR(s.landing_page_url, '[?&]utm_campaign=([^&#]+)', 1, 1, 'e', 1)
    ) AS utm_campaign
  FROM sessions AS s
),

exact_matches AS (
  SELECT
    p.session_id,
    MIN(m.channel_code) AS channel_code,
    BOOLOR_AGG(m.is_paid) AS is_paid
  FROM parsed AS p
  INNER JOIN channel_mapping AS m
    ON p.utm_source = m.utm_source
    AND p.utm_medium = m.utm_medium
  GROUP BY
    p.session_id
),

medium_matches AS (
  SELECT
    p.session_id,
    MIN(m.channel_code) AS channel_code,
    BOOLOR_AGG(m.is_paid) AS is_paid
  FROM parsed AS p
  INNER JOIN channel_mapping AS m
    ON p.utm_medium = m.utm_medium
  WHERE m.utm_source = '-'
  GROUP BY
    p.session_id
),

classified AS (
  SELECT
    p.session_id,
    p.anonymous_id,
    p.customer_id,
    p.session_start_utc,
    p.utm_source,
    p.utm_medium,
    p.utm_campaign,
    COALESCE(
      e.channel_code,
      mm.channel_code,
      IFF(p.referrer_host IS NULL OR p.referrer_host LIKE '%duckshop%', 'DIRECT', 'REFERRAL')
    ) AS channel_code,
    COALESCE(e.is_paid, mm.is_paid, FALSE) AS is_paid
  FROM parsed AS p
  LEFT JOIN exact_matches AS e
    ON p.session_id = e.session_id
  LEFT JOIN medium_matches AS mm
    ON p.session_id = mm.session_id
),

final AS (
  SELECT
    c.session_id,
    c.anonymous_id,
    c.customer_id,
    c.session_start_utc,
    c.utm_source::VARCHAR AS utm_source,
    c.utm_medium::VARCHAR AS utm_medium,
    c.utm_campaign::VARCHAR AS utm_campaign,
    c.channel_code::VARCHAR AS channel_code,
    c.is_paid::BOOLEAN AS is_paid,
    FIRST_VALUE(c.channel_code) OVER (
      PARTITION BY c.anonymous_id
      ORDER BY c.session_start_utc, c.session_id
    )::VARCHAR AS first_touch_channel,
    LAST_VALUE(c.channel_code) OVER (
      PARTITION BY c.anonymous_id
      ORDER BY c.session_start_utc, c.session_id
      ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING
    )::VARCHAR AS last_touch_channel,
    ROW_NUMBER() OVER (
      PARTITION BY c.anonymous_id
      ORDER BY c.session_start_utc, c.session_id
    )::INT AS touch_number
  FROM classified AS c
)

SELECT *
FROM final
