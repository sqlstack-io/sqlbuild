MODEL (
  description """
    One row per reconciliation check with the last earlier key text of its group that holds a
    comma, read with IFF inside LAST_VALUE ... IGNORE NULLS
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    last_listed_key (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    LAST_VALUE(
      IFF(CONTAINS(a.key_text, ','), a.key_text, NULL) IGNORE NULLS
    ) OVER (
      PARTITION BY a.group_code
      ORDER BY a.check_id
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    )::VARCHAR AS last_listed_key
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
