MODEL (
  description """
    One row per reconciliation check and value kind, a FLOAT ratio and its key text both held as
    VARIANT in one UNION ALL column
  """,
  tags [edge_case],
  columns (
    value_row_id (type VARCHAR, nullable false, audits [not_null, unique]),
    value_text (type VARCHAR),
  ),
);

WITH ratios AS (
  SELECT a.check_id || '~ratio' AS value_row_id, TO_VARIANT(a.ratio) AS held_value
  FROM __ref("stg_edge_amount_checks") AS a
),

keys AS (
  SELECT a.check_id || '~key' AS value_row_id, TO_VARIANT(a.key_text) AS held_value
  FROM __ref("stg_edge_amount_checks") AS a
),

held AS (
  SELECT r.value_row_id, r.held_value FROM ratios AS r
  UNION ALL
  SELECT k.value_row_id, k.held_value FROM keys AS k
),

final AS (
  SELECT
    u.value_row_id,
    u.held_value::VARCHAR AS value_text
  FROM held AS u
)

SELECT *
FROM final
