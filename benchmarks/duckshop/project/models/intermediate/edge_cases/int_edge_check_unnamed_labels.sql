MODEL (
  description """
    One row per reconciliation check with a fixed label left unaliased, read back by the name
    Snowflake gives it
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    label (type VARCHAR),
  ),
);

WITH unnamed AS (
  -- No alias: Snowflake names this column 'FIXED', quotes included.
  SELECT
    a.check_id,
    'fixed'
  FROM __ref("stg_edge_amount_checks") AS a
),

final AS (
  SELECT
    u.check_id,
    u."'FIXED'"::VARCHAR AS label
  FROM unnamed AS u
)

SELECT *
FROM final
