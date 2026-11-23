MODEL (
  description """
    One row per reconciliation check with its wide key read as epoch milliseconds, and whether a
    zoned gateway time falls before date text
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    key_as_time (type TIMESTAMP_NTZ),
    has_early_callback (type BOOLEAN),
  ),
);

WITH keys AS (
  SELECT
    a.check_id,
    TO_TIMESTAMP(a.wide_key, 3)::TIMESTAMP_NTZ AS key_as_time
  FROM __ref("stg_edge_amount_checks") AS a
),

early AS (
  SELECT COUNT(*) AS early_count
  FROM __ref("stg_edge_gateway_callbacks") AS g
  WHERE g.event_at < '2026-03-01'
),

final AS (
  SELECT
    k.check_id,
    k.key_as_time,
    (e.early_count > 0)::BOOLEAN AS has_early_callback
  FROM keys AS k
  CROSS JOIN early AS e
)

SELECT *
FROM final
