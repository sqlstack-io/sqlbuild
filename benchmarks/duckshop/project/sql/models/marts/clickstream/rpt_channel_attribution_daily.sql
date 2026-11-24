MODEL (
  description """
    One row per day and channel with sessions, conversions and first-touch versus last-touch credit
  """,
  columns (
    channel_day_key (type VARCHAR, nullable false, audits [not_null, unique]),
    session_date (type DATE),
    channel_code (type VARCHAR),
    sessions (type INT),
    visitors (type INT),
    conversions (type INT),
    conversion_rate (type NUMBER(9, 4)),
    first_touch_credit (type INT),
    paid_share (type NUMBER(9, 4)),
  ),
);

WITH sessions AS (
  SELECT *
  FROM __ref("fct_web_sessions")
),

channel_days AS (
  SELECT
    s.session_start_utc::DATE AS session_date,
    COALESCE(s.channel_code, 'UNKNOWN') AS channel_code,
    s.session_id,
    s.anonymous_id,
    s.converted,
    s.is_paid,
    'last_touch' AS credit_basis
  FROM sessions AS s
  UNION ALL
  SELECT
    s.session_start_utc::DATE AS session_date,
    COALESCE(s.first_touch_channel, 'UNKNOWN') AS channel_code,
    s.session_id,
    s.anonymous_id,
    s.converted,
    s.is_paid,
    'first_touch' AS credit_basis
  FROM sessions AS s
  WHERE s.converted
),

final AS (
  SELECT
    (
      TO_VARCHAR(c.session_date, 'YYYY-MM-DD') || ':' || c.channel_code
    )::VARCHAR AS channel_day_key,
    c.session_date::DATE AS session_date,
    c.channel_code::VARCHAR AS channel_code,
    COUNT_IF(c.credit_basis = 'last_touch')::INT AS sessions,
    COUNT(DISTINCT IFF(c.credit_basis = 'last_touch', c.anonymous_id, NULL))::INT AS visitors,
    COUNT_IF(c.credit_basis = 'last_touch' AND c.converted)::INT AS conversions,
    ROUND(
      COUNT_IF(c.credit_basis = 'last_touch' AND c.converted)
        / NULLIF(COUNT_IF(c.credit_basis = 'last_touch'), 0),
      4
    )::NUMBER(9, 4) AS conversion_rate,
    COUNT(
      DISTINCT IFF(c.credit_basis = 'first_touch', c.session_id, NULL)
    )::INT AS first_touch_credit,
    ROUND(
      AVG(IFF(c.credit_basis = 'last_touch', IFF(c.is_paid, 1, 0), NULL)),
      4
    )::NUMBER(9, 4) AS paid_share
  FROM channel_days AS c
  GROUP BY
    c.session_date,
    c.channel_code
)

SELECT *
FROM final
