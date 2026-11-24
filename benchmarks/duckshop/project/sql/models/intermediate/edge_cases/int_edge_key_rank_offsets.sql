MODEL (
  description """
    One row per reconciliation check with a rank one past its wide key or a fallback, and its
    wide key read back with TRY_TO_NUMBER
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    candidate_rank (type INT),
    wide_key_number (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    1 + COALESCE(a.wide_key, 999) AS candidate_rank,
    TRY_TO_NUMBER(a.wide_key)::NUMBER(38, 0) AS wide_key_number
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
