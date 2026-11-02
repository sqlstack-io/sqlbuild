MODEL (
  description """
    One row per reconciliation check with the number of key texts in its group, from a windowed
    ARRAY_AGG
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    group_key_count (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    ARRAY_SIZE(ARRAY_AGG(a.key_text) OVER (PARTITION BY a.group_code))::NUMBER(38, 0) AS group_key_count
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
