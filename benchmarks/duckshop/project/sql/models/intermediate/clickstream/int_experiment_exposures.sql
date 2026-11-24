MODEL (
  description """
    One row per experiment assignment with the visitor's sessions and conversions after assignment,
    to measure each variant against control.
  """,
  columns (
    assignment_id (type VARCHAR, nullable false, audits [not_null, unique]),
    experiment_key (type VARCHAR, audits [not_null]),
    variant_key (type VARCHAR, audits [not_null]),
    is_control (type BOOLEAN),
    anonymous_id (type VARCHAR),
    assigned_at_utc (type TIMESTAMP_NTZ),
    sessions_after (type INT),
    converted (type BOOLEAN),
    days_to_convert (type NUMBER(10, 2)),
    assignment_surface (type VARCHAR),
  ),
);

WITH assignments AS (
  SELECT *
  FROM __ref("stg_experiment_assignments")
),

sessions AS (
  SELECT *
  FROM __ref("int_web_sessions")
),

assigned AS (
  SELECT
    a.assignment_id,
    a.experiment_key,
    a.variant_key,
    a.anonymous_id,
    a.assignment_surface,
    CONVERT_TIMEZONE('UTC', a.assigned_at)::TIMESTAMP_NTZ AS assigned_at_utc
  FROM assignments AS a
),

after_assignment AS (
  SELECT
    x.assignment_id,
    COUNT(s.session_id) AS sessions_after,
    MIN(IFF(s.has_order, s.session_start_utc, NULL)) AS first_order_session_utc
  FROM assigned AS x
  LEFT JOIN sessions AS s
    ON x.anonymous_id = s.anonymous_id
    AND s.session_start_utc >= x.assigned_at_utc
  GROUP BY
    x.assignment_id
),

final AS (
  SELECT
    x.assignment_id,
    x.experiment_key,
    x.variant_key,
    (
      x.variant_key IN ('control', 'a', 'baseline')
    ) AS is_control,
    x.anonymous_id,
    x.assigned_at_utc,
    COALESCE(f.sessions_after, 0)::INT AS sessions_after,
    (
      f.first_order_session_utc IS NOT NULL
    ) AS converted,
    ROUND(
      DATEDIFF('minute', x.assigned_at_utc, f.first_order_session_utc) / 1440,
      2
    )::NUMBER(10, 2) AS days_to_convert,
    x.assignment_surface
  FROM assigned AS x
  LEFT JOIN after_assignment AS f
    ON x.assignment_id = f.assignment_id
)

SELECT *
FROM final
